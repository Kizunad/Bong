use bong_server::combat::components::*;
use bong_server::combat::events::*;
use bong_server::combat::lifecycle::*;
use bong_server::combat::CombatClock;
use bong_server::cultivation::components::*;
use bong_server::cultivation::death_hooks::*;
use bong_server::cultivation::life_record::*;
use bong_server::cultivation::lifespan::*;
use bong_server::cultivation::tribulation::*;
use bong_server::inventory::*;
use bong_server::network::redis_bridge::*;
use bong_server::network::vfx_event_emit::*;
use bong_server::network::*;
use bong_server::persistence::*;
use bong_server::qi_physics::constants::*;
use bong_server::qi_physics::*;
use bong_server::schema::server_data::*;
use bong_server::schema::tribulation::*;
use bong_server::schema::vfx_event::*;
use bong_server::skill::events::*;
use bong_server::world::dimension::*;
use bong_server::world::zone::*;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use valence::prelude::{
    bevy_ecs, App, BlockPos, Client, Entity, Events, IntoSystemConfigs, Position, Update, Username,
};
use valence::testing::create_mock_client;

fn test_inventory(items: Vec<ItemInstance>, bone_coins: u64) -> PlayerInventory {
    PlayerInventory {
        triggered_treasures: Vec::new(),
        revision: InventoryRevision(1),
        containers: vec![ContainerState {
            quick_access: false,
            id: MAIN_PACK_CONTAINER_ID.to_string(),
            name: "主背包".to_string(),
            rows: 5,
            cols: 5,
            items: items
                .into_iter()
                .enumerate()
                .map(|(idx, instance)| PlacedItemState {
                    row: (idx / 5) as u8,
                    col: (idx % 5) as u8,
                    instance,
                })
                .collect(),
            owner_instance_id: None,
        }],
        equipped: Default::default(),
        hotbar: Default::default(),
        bone_coins,
        max_weight: 50.0,
    }
}
fn make_settled_event_with_source(
    entity: Entity,
    outcome: DuXuOutcomeV1,
    source: JueBiTriggerSource,
) -> TribulationSettled {
    TribulationSettled {
        entity,
        kind: TribulationKind::JueBi,
        source: Some(source),
        result: DuXuResultV1 {
            char_id: "halfstep_test_char".to_string(),
            outcome,
            killer: None,
            waves_survived: 3,
            reason: Some("halfstep_test".to_string()),
        },
    }
}
fn p0_metrics_test_app() -> App {
    let mut app = qi_test_app();
    app.insert_resource(CombatClock::default());
    app.init_resource::<TribulationMetrics>();
    app.init_resource::<QuotaFullTracker>();
    app.init_resource::<HalfStepRechallengeQueue>();
    // plan-zone-qi-economy-v1 P0 §8.1 决议 #2：DEFAULT_VOID_QUOTA_K 随
    // DEFAULT_SPIRIT_QI_TOTAL 等比例缩放，"满预算 → quota_limit=2" 这条不变式
    // 只有在预算取 DEFAULT_SPIRIT_QI_TOTAL 本身时才成立——不能再写死旧尺度下的
    // 字面量 100.0（缩放后那只是全局预算的 0.5%，quota_limit 会跌到 0）。
    app.insert_resource(WorldQiBudget::from_total(DEFAULT_SPIRIT_QI_TOTAL));
    app.insert_resource(VoidQuotaConfig::default());
    app.add_event::<TribulationSettled>();
    app.add_event::<AscensionQuotaOpened>();
    app.add_event::<AscensionQuotaOccupied>();
    app.add_event::<QiTransfer>();
    app.add_event::<HalfStepRechallengeTriggerEvent>();
    app.add_systems(
        Update,
        (
            track_tribulation_metrics_system,
            track_quota_full_duration_system,
            dispatch_rechallenge_on_quota_opened_system,
        ),
    );
    app
}
fn drain_rechallenge_triggers(app: &mut App) -> Vec<HalfStepRechallengeTriggerEvent> {
    app.world_mut()
        .resource_mut::<Events<HalfStepRechallengeTriggerEvent>>()
        .drain()
        .collect()
}
fn unique_temp_dir(test_name: &str) -> PathBuf {
    let unique_suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "bong-tribulation-{test_name}-{}-{unique_suffix}",
        std::process::id()
    ))
}
fn persistence_settings(test_name: &str) -> (PersistenceSettings, PathBuf) {
    let root = unique_temp_dir(test_name);
    let db_path = root.join("data").join("bong.db");
    bootstrap_sqlite(&db_path, &format!("tribulation-{test_name}"))
        .expect("sqlite bootstrap should succeed");
    (
        PersistenceSettings::with_db_path(&db_path, format!("tribulation-{test_name}")),
        root,
    )
}
fn spawn_halfstep_candidate(
    app: &mut App,
    initial_qi_max: f64,
    initial_lifespan_cap: u32,
) -> Entity {
    app.world_mut()
        .spawn((
            Cultivation {
                realm: Realm::Spirit,
                qi_current: 0.0,
                qi_max: initial_qi_max,
                ..Default::default()
            },
            LifespanComponent::new(initial_lifespan_cap),
        ))
        .id()
}
fn collect_qi_transfers(app: &mut App) -> Vec<QiTransfer> {
    app.world_mut()
        .resource_mut::<Events<QiTransfer>>()
        .drain()
        .collect()
}
fn unbootstrapped_persistence_settings(test_name: &str) -> (PersistenceSettings, PathBuf) {
    let root = unique_temp_dir(test_name);
    let db_path = root.join("data").join("bong.db");
    (
        PersistenceSettings::with_db_path(
            &db_path,
            format!("tribulation-unbootstrapped-{test_name}"),
        ),
        root,
    )
}
fn qi_test_app() -> App {
    let mut app = App::new();
    app.insert_resource(WorldQiAccount::default());
    app
}
fn collect_vfx_payloads(app: &mut App) -> Vec<VfxEventPayloadV1> {
    app.world_mut()
        .resource_mut::<Events<VfxEventRequest>>()
        .drain()
        .map(|event| event.payload)
        .collect()
}
fn make_settled_event(entity: Entity, outcome: DuXuOutcomeV1) -> TribulationSettled {
    make_settled_event_with_source(entity, outcome, JueBiTriggerSource::VoidQuotaExceeded)
}
fn all_meridians_open() -> MeridianSystem {
    let mut meridians = MeridianSystem::default();
    for (idx, id) in MeridianId::REGULAR
        .iter()
        .chain(MeridianId::EXTRAORDINARY.iter())
        .enumerate()
    {
        let meridian = meridians.get_mut(*id);
        meridian.opened = true;
        meridian.open_progress = 1.0;
        meridian.opened_at = idx as u64;
    }
    meridians
}
fn test_item(instance_id: u64) -> ItemInstance {
    ItemInstance {
        instance_id,
        template_id: format!("test_item_{instance_id}"),
        display_name: format!("test {instance_id}"),
        grid_w: 1,
        grid_h: 1,
        weight: 0.5,
        rarity: ItemRarity::Common,
        description: "test".to_string(),
        stack_count: 1,
        spirit_quality: 1.0,
        durability: 1.0,
        freshness: None,
        mineral_id: None,
        charges: None,
        forge_quality: None,
        forge_color: None,
        forge_side_effects: Vec::new(),
        forge_achieved_tier: None,
        alchemy: None,
        lingering_owner_qi: None,
    }
}

#[test]
fn omen_to_lock_emits_lock_event() {
    let mut app = qi_test_app();
    app.insert_resource(CombatClock {
        tick: DUXU_OMEN_TICKS,
    });
    app.add_event::<TribulationLocked>();
    app.add_event::<TribulationWaveCleared>();
    app.add_systems(Update, tribulation_phase_tick_system);

    let entity = app
        .world_mut()
        .spawn((
            Lifecycle {
                character_id: "offline:Azure".to_string(),
                ..Default::default()
            },
            Username("Azure".to_string()),
            TribulationState {
                kind: TribulationKind::DuXu,
                phase: TribulationPhase::Omen,
                epicenter: [12.0, 66.0, -8.0],
                wave_current: 0,
                waves_total: 3,
                started_tick: 0,
                phase_started_tick: 0,
                next_wave_tick: DUXU_OMEN_TICKS + DUXU_LOCK_TICKS,
                participants: vec!["offline:Azure".to_string()],
                failed: false,
            },
        ))
        .id();

    app.update();

    let state = app
        .world()
        .get::<TribulationState>(entity)
        .expect("tribulation should remain active");
    assert_eq!(state.phase, TribulationPhase::Lock);

    let events = app.world().resource::<Events<TribulationLocked>>();
    let emitted: Vec<_> = events.get_reader().read(events).cloned().collect();
    assert_eq!(emitted.len(), 1);
    assert_eq!(emitted[0].char_id, "offline:Azure");
    assert_eq!(emitted[0].actor_name, "Azure");
    assert_eq!(emitted[0].epicenter, [12.0, 66.0, -8.0]);
    assert_eq!(emitted[0].waves_total, 3);
}
#[test]
fn start_tribulation_system_dedupes_same_tick_internal_events() {
    let mut app = qi_test_app();
    let (settings, root) = persistence_settings("start-tribulation-dedupe");
    app.insert_resource(settings);
    app.insert_resource(WorldQiBudget::from_total(100.0));
    app.insert_resource(VoidQuotaConfig::default());
    app.add_event::<InitiateXuhuaTribulation>();
    app.add_event::<TribulationAnnounce>();
    app.add_event::<TribulationSettled>();
    app.add_event::<CultivationDeathTrigger>();
    app.add_event::<VfxEventRequest>();
    app.add_systems(Update, start_tribulation_system);
    let (client_bundle, _helper) = create_mock_client("Azure");
    let entity = app
        .world_mut()
        .spawn((
            client_bundle,
            Cultivation {
                realm: Realm::Spirit,
                qi_current: 210.0,
                qi_max: 210.0,
                ..Default::default()
            },
            all_meridians_open(),
            CurrentDimension(DimensionKind::Tsy),
            Lifecycle {
                character_id: "offline:Azure".to_string(),
                ..Default::default()
            },
        ))
        .id();
    app.world_mut()
        .entity_mut(entity)
        .insert(Position::new([12.0, 66.0, -8.0]));

    for _ in 0..2 {
        app.world_mut().send_event(InitiateXuhuaTribulation {
            entity,
            waves_total: 3,
            started_tick: 100,
        });
    }
    app.update();

    let state = app
        .world()
        .get::<TribulationState>(entity)
        .expect("tribulation should start once");
    assert_eq!(state.phase, TribulationPhase::Omen);
    assert_eq!(state.started_tick, 100);
    let origin = app
        .world()
        .get::<TribulationOriginDimension>(entity)
        .expect("tribulation should remember origin dimension");
    assert_eq!(origin.0, DimensionKind::Tsy);
    let announce = app.world().resource::<Events<TribulationAnnounce>>();
    let emitted: Vec<_> = announce.get_reader().read(announce).cloned().collect();
    assert_eq!(emitted.len(), 1);
    assert_eq!(emitted[0].entity, entity);
    assert_eq!(emitted[0].actor_name, "Azure");
    assert_eq!(app.world().resource::<Events<VfxEventRequest>>().len(), 1);

    let _ = fs::remove_dir_all(root);
}
#[test]
fn start_tribulation_system_reserves_void_quota_fcfs_within_tick() {
    let mut app = qi_test_app();
    let (settings, root) = persistence_settings("start-tribulation-quota-fcfs");
    app.insert_resource(settings);
    app.insert_resource(WorldQiBudget::from_total(50.0));
    app.insert_resource(VoidQuotaConfig { quota_k: 50.0 });
    app.add_event::<InitiateXuhuaTribulation>();
    app.add_event::<TribulationAnnounce>();
    app.add_event::<TribulationSettled>();
    app.add_event::<CultivationDeathTrigger>();
    app.add_event::<VfxEventRequest>();
    app.add_systems(Update, start_tribulation_system);

    let first = app
        .world_mut()
        .spawn((
            Cultivation {
                realm: Realm::Spirit,
                qi_current: 210.0,
                qi_max: 210.0,
                ..Default::default()
            },
            all_meridians_open(),
            Lifecycle {
                character_id: "offline:Azure".to_string(),
                ..Default::default()
            },
            Position::new([12.0, 66.0, -8.0]),
        ))
        .id();
    let second = app
        .world_mut()
        .spawn((
            Cultivation {
                realm: Realm::Spirit,
                qi_current: 210.0,
                qi_max: 210.0,
                ..Default::default()
            },
            all_meridians_open(),
            Lifecycle {
                character_id: "offline:Beryl".to_string(),
                ..Default::default()
            },
            Position::new([16.0, 66.0, -8.0]),
        ))
        .id();

    app.world_mut().send_event(InitiateXuhuaTribulation {
        entity: first,
        waves_total: 3,
        started_tick: 100,
    });
    app.world_mut().send_event(InitiateXuhuaTribulation {
        entity: second,
        waves_total: 3,
        started_tick: 100,
    });

    app.update();

    let first_state = app
        .world()
        .get::<TribulationState>(first)
        .expect("first tribulation should start");
    assert_eq!(first_state.kind, TribulationKind::DuXu);
    let second_state = app
        .world()
        .get::<TribulationState>(second)
        .expect("second over-quota DuXu should start before appending JueBi");
    assert_eq!(second_state.kind, TribulationKind::DuXu);
    assert!(
        app.world().get::<JueBiAfterDuXuQuota>(second).is_some(),
        "second over-quota DuXu should carry a JueBi follow-up marker"
    );
    let settled = app.world().resource::<Events<TribulationSettled>>();
    let settled_events: Vec<_> = settled.get_reader().read(settled).cloned().collect();
    assert!(
        settled_events.is_empty(),
        "over-quota DuXu should no longer settle as instant quota death"
    );
    let death_triggers = app.world().resource::<Events<CultivationDeathTrigger>>();
    let deaths: Vec<_> = death_triggers
        .get_reader()
        .read(death_triggers)
        .cloned()
        .collect();
    assert!(
        deaths.is_empty(),
        "over-quota DuXu should no longer emit instant quota death"
    );

    let _ = fs::remove_dir_all(root);
}
#[test]
fn start_tribulation_system_counts_in_flight_void_tribulations_across_ticks() {
    let mut app = qi_test_app();
    let (settings, root) = persistence_settings("start-tribulation-quota-cross-tick");
    app.insert_resource(settings);
    app.insert_resource(WorldQiBudget::from_total(50.0));
    app.insert_resource(VoidQuotaConfig { quota_k: 50.0 });
    app.add_event::<InitiateXuhuaTribulation>();
    app.add_event::<TribulationAnnounce>();
    app.add_event::<TribulationSettled>();
    app.add_event::<CultivationDeathTrigger>();
    app.add_event::<VfxEventRequest>();
    app.add_systems(Update, start_tribulation_system);

    let first = app
        .world_mut()
        .spawn((
            Cultivation {
                realm: Realm::Spirit,
                qi_current: 210.0,
                qi_max: 210.0,
                ..Default::default()
            },
            all_meridians_open(),
            Lifecycle {
                character_id: "offline:Azure".to_string(),
                ..Default::default()
            },
            Position::new([12.0, 66.0, -8.0]),
        ))
        .id();
    let second = app
        .world_mut()
        .spawn((
            Cultivation {
                realm: Realm::Spirit,
                qi_current: 210.0,
                qi_max: 210.0,
                ..Default::default()
            },
            all_meridians_open(),
            Lifecycle {
                character_id: "offline:Beryl".to_string(),
                ..Default::default()
            },
            Position::new([16.0, 66.0, -8.0]),
        ))
        .id();

    app.world_mut().send_event(InitiateXuhuaTribulation {
        entity: first,
        waves_total: 3,
        started_tick: 100,
    });
    app.update();
    assert!(
        app.world().get::<TribulationState>(first).is_some(),
        "first in-flight tribulation should reserve the only void slot"
    );

    app.world_mut().send_event(InitiateXuhuaTribulation {
        entity: second,
        waves_total: 3,
        started_tick: 200,
    });
    app.update();

    assert!(
        app.world().get::<TribulationState>(second).is_some(),
        "later tick over-quota starter should enter DuXu and append JueBi at settlement"
    );
    assert!(
        app.world().get::<JueBiAfterDuXuQuota>(second).is_some(),
        "later tick over-quota starter should carry a JueBi follow-up marker"
    );
    let settled = app.world().resource::<Events<TribulationSettled>>();
    let settled_events: Vec<_> = settled.get_reader().read(settled).cloned().collect();
    assert!(
        settled_events.is_empty(),
        "over-quota DuXu should defer settlement until JueBi resolves"
    );

    let _ = fs::remove_dir_all(root);
}
#[test]
fn start_tribulation_system_fails_closed_when_quota_store_unreadable() {
    let mut app = qi_test_app();
    let (settings, root) =
        unbootstrapped_persistence_settings("start-tribulation-quota-read-failure");
    app.insert_resource(settings);
    app.insert_resource(WorldQiBudget::from_total(100.0));
    app.insert_resource(VoidQuotaConfig::default());
    app.add_event::<InitiateXuhuaTribulation>();
    app.add_event::<TribulationAnnounce>();
    app.add_event::<TribulationSettled>();
    app.add_event::<CultivationDeathTrigger>();
    app.add_event::<VfxEventRequest>();
    app.add_systems(Update, start_tribulation_system);

    let entity = app
        .world_mut()
        .spawn((
            Cultivation {
                realm: Realm::Spirit,
                qi_current: 210.0,
                qi_max: 210.0,
                ..Default::default()
            },
            all_meridians_open(),
            Lifecycle {
                character_id: "offline:Azure".to_string(),
                ..Default::default()
            },
            Position::new([12.0, 66.0, -8.0]),
        ))
        .id();

    app.world_mut().send_event(InitiateXuhuaTribulation {
        entity,
        waves_total: 3,
        started_tick: 100,
    });
    app.update();

    assert!(
        app.world().get::<TribulationState>(entity).is_none(),
        "quota store read failure must not start or reserve an in-memory tribulation"
    );
    assert_eq!(
        app.world().resource::<Events<TribulationAnnounce>>().len(),
        0
    );
    assert_eq!(
        app.world().resource::<Events<TribulationSettled>>().len(),
        0
    );
    assert_eq!(
        app.world()
            .resource::<Events<CultivationDeathTrigger>>()
            .len(),
        0
    );
    assert_eq!(app.world().resource::<Events<VfxEventRequest>>().len(), 0);

    let _ = fs::remove_dir_all(root);
}
#[test]
fn start_tribulation_system_aborts_when_active_row_persist_fails() {
    let mut app = qi_test_app();
    let (settings, root) = persistence_settings("start-tribulation-active-row-persist-failure");
    {
        let connection =
            rusqlite::Connection::open(settings.db_path()).expect("sqlite should open");
        connection
            .execute_batch(
                "
                DROP TABLE tribulations_active;
                CREATE VIEW tribulations_active AS
                SELECT
                    'offline:Existing' AS char_id,
                    0 AS wave_current,
                    3 AS waves_total,
                    0 AS started_tick,
                    1 AS schema_version,
                    0 AS last_updated_wall
                WHERE 0;
                ",
            )
            .expect("active tribulation view should be installed");
    }
    app.insert_resource(settings);
    app.insert_resource(WorldQiBudget::from_total(100.0));
    app.insert_resource(VoidQuotaConfig::default());
    app.add_event::<InitiateXuhuaTribulation>();
    app.add_event::<TribulationAnnounce>();
    app.add_event::<TribulationSettled>();
    app.add_event::<CultivationDeathTrigger>();
    app.add_event::<VfxEventRequest>();
    app.add_systems(Update, start_tribulation_system);

    let entity = app
        .world_mut()
        .spawn((
            Cultivation {
                realm: Realm::Spirit,
                qi_current: 210.0,
                qi_max: 210.0,
                ..Default::default()
            },
            all_meridians_open(),
            Lifecycle {
                character_id: "offline:Azure".to_string(),
                ..Default::default()
            },
            Position::new([12.0, 66.0, -8.0]),
        ))
        .id();

    app.world_mut().send_event(InitiateXuhuaTribulation {
        entity,
        waves_total: 3,
        started_tick: 100,
    });
    app.update();

    assert!(
        app.world().get::<TribulationState>(entity).is_none(),
        "active-row persist failure must not start an untracked tribulation"
    );
    assert_eq!(
        app.world().resource::<Events<TribulationAnnounce>>().len(),
        0
    );
    assert_eq!(
        app.world().resource::<Events<TribulationSettled>>().len(),
        0
    );
    assert_eq!(
        app.world()
            .resource::<Events<CultivationDeathTrigger>>()
            .len(),
        0
    );
    assert_eq!(app.world().resource::<Events<VfxEventRequest>>().len(), 0);

    let _ = fs::remove_dir_all(root);
}
#[test]
fn tribulation_wave_system_aborts_ascension_when_quota_write_fails() {
    let mut app = qi_test_app();
    let (settings, root) =
        unbootstrapped_persistence_settings("tribulation-ascension-quota-write-failure");
    app.insert_resource(settings);
    app.add_event::<TribulationWaveCleared>();
    app.add_event::<SkillCapChanged>();
    app.add_event::<TribulationSettled>();
    app.add_event::<AscensionQuotaOccupied>();
    app.add_event::<JueBiTriggeredEvent>();
    app.add_systems(Update, tribulation_wave_system);

    let entity = app
        .world_mut()
        .spawn((
            Cultivation {
                realm: Realm::Spirit,
                qi_current: 210.0,
                qi_max: 210.0,
                ..Default::default()
            },
            all_meridians_open(),
            Lifecycle {
                character_id: "offline:Azure".to_string(),
                ..Default::default()
            },
            LifespanComponent::new(LifespanCapTable::SPIRIT),
            TribulationState {
                kind: TribulationKind::DuXu,
                phase: TribulationPhase::Wave(3),
                epicenter: [0.0, 66.0, 0.0],
                wave_current: 2,
                waves_total: 3,
                started_tick: 100,
                phase_started_tick: 200,
                next_wave_tick: 300,
                participants: vec!["offline:Azure".to_string()],
                failed: false,
            },
        ))
        .id();

    app.world_mut()
        .send_event(TribulationWaveCleared { entity, wave: 3 });
    app.update();

    let cultivation = app
        .world()
        .get::<Cultivation>(entity)
        .expect("cultivation should remain attached");
    assert_eq!(cultivation.realm, Realm::Spirit);
    assert_eq!(cultivation.qi_max, 210.0);
    let lifespan = app
        .world()
        .get::<LifespanComponent>(entity)
        .expect("lifespan should remain attached");
    assert_eq!(lifespan.cap_by_realm, LifespanCapTable::SPIRIT);
    let state = app
        .world()
        .get::<TribulationState>(entity)
        .expect("failed quota write should keep tribulation state for operator recovery");
    assert_ne!(state.phase, TribulationPhase::Settle);
    assert_eq!(
        app.world()
            .resource::<Events<AscensionQuotaOccupied>>()
            .len(),
        0
    );
    assert_eq!(app.world().resource::<Events<SkillCapChanged>>().len(), 0);
    assert_eq!(
        app.world().resource::<Events<TribulationSettled>>().len(),
        0
    );

    let _ = fs::remove_dir_all(root);
}
#[test]
fn tribulation_announce_emits_boundary_vfx() {
    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 0 });
    app.add_event::<TribulationAnnounce>();
    app.add_event::<TribulationLocked>();
    app.add_event::<TribulationWaveCleared>();
    app.add_event::<JueBiTriggeredEvent>();
    app.add_event::<VfxEventRequest>();
    app.add_systems(Update, emit_tribulation_boundary_vfx_system);

    app.world_mut().send_event(TribulationAnnounce {
        entity: Entity::PLACEHOLDER,
        char_id: "offline:Azure".to_string(),
        actor_name: "Azure".to_string(),
        epicenter: [12.0, 66.0, -8.0],
        waves_total: 3,
        started_tick: 0,
    });

    app.update();

    let payloads = collect_vfx_payloads(&mut app);
    assert_eq!(payloads.len(), 2);
    match &payloads[0] {
        VfxEventPayloadV1::SpawnParticle {
            event_id,
            origin,
            direction,
            count,
            duration_ticks,
            ..
        } => {
            assert_eq!(event_id, DUXU_OMEN_CLOUD_VFX_EVENT_ID);
            assert_eq!(*origin, [12.0, 90.0, -8.0]);
            assert_eq!(*direction, Some([24.0, 8.0, 24.0]));
            assert_eq!(*count, Some(36));
            assert_eq!(*duration_ticks, Some(200));
        }
        other => panic!("unexpected omen cloud vfx payload: {other:?}"),
    }
    match &payloads[1] {
        VfxEventPayloadV1::SpawnParticle {
            event_id,
            origin,
            strength,
            duration_ticks,
            ..
        } => {
            assert_eq!(event_id, DUXU_BOUNDARY_VFX_EVENT_ID);
            assert_eq!(*origin, [12.0, 66.0, -8.0]);
            assert_eq!(*strength, Some(1.0));
            assert_eq!(*duration_ticks, Some(200));
        }
        other => panic!("unexpected boundary vfx payload: {other:?}"),
    }
}
#[test]
fn omen_midpoint_emits_soft_boundary_once() {
    let mut app = qi_test_app();
    app.insert_resource(CombatClock {
        tick: DUXU_OMEN_TICKS / 2,
    });
    app.add_event::<TribulationAnnounce>();
    app.add_event::<TribulationLocked>();
    app.add_event::<TribulationWaveCleared>();
    app.add_event::<JueBiTriggeredEvent>();
    app.add_event::<VfxEventRequest>();
    app.add_systems(Update, emit_tribulation_boundary_vfx_system);

    app.world_mut().spawn(TribulationState {
        kind: TribulationKind::DuXu,
        phase: TribulationPhase::Omen,
        epicenter: [0.0, 66.0, 0.0],
        wave_current: 0,
        waves_total: 3,
        started_tick: 0,
        phase_started_tick: 0,
        next_wave_tick: DUXU_OMEN_TICKS + DUXU_LOCK_TICKS,
        participants: vec!["offline:Azure".to_string()],
        failed: false,
    });

    app.update();
    app.update();

    let payloads = collect_vfx_payloads(&mut app);
    assert_eq!(payloads.len(), 1);
    match &payloads[0] {
        VfxEventPayloadV1::SpawnParticle { strength, .. } => {
            assert_eq!(*strength, Some(0.5));
        }
        other => panic!("unexpected boundary vfx payload: {other:?}"),
    }
}
#[test]
fn lock_and_wave_events_emit_boundary_vfx() {
    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 900 });
    app.add_event::<TribulationAnnounce>();
    app.add_event::<TribulationLocked>();
    app.add_event::<TribulationWaveCleared>();
    app.add_event::<JueBiTriggeredEvent>();
    app.add_event::<VfxEventRequest>();
    app.add_systems(Update, emit_tribulation_boundary_vfx_system);

    let entity = app
        .world_mut()
        .spawn(TribulationState {
            kind: TribulationKind::DuXu,
            phase: TribulationPhase::Wave(1),
            epicenter: [0.0, 66.0, 0.0],
            wave_current: 1,
            waves_total: 3,
            started_tick: 0,
            phase_started_tick: 900,
            next_wave_tick: 1200,
            participants: vec!["offline:Azure".to_string()],
            failed: false,
        })
        .id();
    app.world_mut().send_event(TribulationLocked {
        entity,
        char_id: "offline:Azure".to_string(),
        actor_name: "Azure".to_string(),
        epicenter: [0.0, 66.0, 0.0],
        waves_total: 3,
    });
    app.world_mut()
        .send_event(TribulationWaveCleared { entity, wave: 1 });

    app.update();

    let strengths: Vec<_> = collect_vfx_payloads(&mut app)
        .into_iter()
        .map(|payload| match payload {
            VfxEventPayloadV1::SpawnParticle { strength, .. } => strength,
            other => panic!("unexpected boundary vfx payload: {other:?}"),
        })
        .collect();
    assert_eq!(strengths, vec![Some(0.2), Some(0.1)]);
}
#[test]
fn start_du_xu_request_rejects_non_spirit_or_incomplete_meridians() {
    let mut app = qi_test_app();
    app.add_event::<StartDuXuRequest>();
    app.add_event::<InitiateXuhuaTribulation>();
    app.add_systems(Update, start_du_xu_request_system);

    let non_spirit = app
        .world_mut()
        .spawn((
            Cultivation {
                realm: Realm::Condense,
                qi_current: 210.0,
                qi_max: 210.0,
                ..Default::default()
            },
            all_meridians_open(),
        ))
        .id();
    let incomplete = app
        .world_mut()
        .spawn((
            Cultivation {
                realm: Realm::Spirit,
                qi_current: 210.0,
                qi_max: 210.0,
                ..Default::default()
            },
            MeridianSystem::default(),
        ))
        .id();

    app.world_mut().send_event(StartDuXuRequest {
        entity: non_spirit,
        requested_at_tick: 100,
    });
    app.world_mut().send_event(StartDuXuRequest {
        entity: incomplete,
        requested_at_tick: 100,
    });
    app.update();

    let events = app.world().resource::<Events<InitiateXuhuaTribulation>>();
    assert!(events.get_reader().read(events).next().is_none());
}
#[test]
fn start_du_xu_request_rejects_already_active_tribulation() {
    let mut app = qi_test_app();
    app.add_event::<StartDuXuRequest>();
    app.add_event::<InitiateXuhuaTribulation>();
    app.add_systems(Update, start_du_xu_request_system);
    let entity = app
        .world_mut()
        .spawn((
            Cultivation {
                realm: Realm::Spirit,
                qi_current: 210.0,
                qi_max: 210.0,
                ..Default::default()
            },
            all_meridians_open(),
            TribulationState {
                kind: TribulationKind::DuXu,
                phase: TribulationPhase::Omen,
                epicenter: [0.0, 66.0, 0.0],
                wave_current: 0,
                waves_total: 3,
                started_tick: 0,
                phase_started_tick: 0,
                next_wave_tick: 0,
                participants: vec!["offline:Azure".to_string()],
                failed: false,
            },
        ))
        .id();

    app.world_mut().send_event(StartDuXuRequest {
        entity,
        requested_at_tick: 100,
    });
    app.update();

    let events = app.world().resource::<Events<InitiateXuhuaTribulation>>();
    assert!(events.get_reader().read(events).next().is_none());
}
#[test]
fn start_du_xu_request_dedupes_same_tick_duplicate_requests() {
    let mut app = qi_test_app();
    app.add_event::<StartDuXuRequest>();
    app.add_event::<InitiateXuhuaTribulation>();
    app.add_systems(Update, start_du_xu_request_system);
    let entity = app
        .world_mut()
        .spawn((
            Cultivation {
                realm: Realm::Spirit,
                qi_current: 210.0,
                qi_max: 210.0,
                ..Default::default()
            },
            all_meridians_open(),
        ))
        .id();

    for _ in 0..2 {
        app.world_mut().send_event(StartDuXuRequest {
            entity,
            requested_at_tick: 100,
        });
    }
    app.update();

    let events = app.world().resource::<Events<InitiateXuhuaTribulation>>();
    let emitted: Vec<_> = events.get_reader().read(events).cloned().collect();
    assert_eq!(emitted.len(), 1);
    assert_eq!(emitted[0].entity, entity);
    assert_eq!(emitted[0].started_tick, 100);
}
#[test]
fn fourth_wave_enters_heart_demon_without_aoe() {
    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 2100 });
    app.add_event::<TribulationLocked>();
    app.add_event::<TribulationWaveCleared>();
    app.add_event::<TribulationFailed>();
    app.add_event::<DeathEvent>();
    app.add_systems(
        Update,
        (
            tribulation_phase_tick_system,
            tribulation_aoe_system.after(tribulation_phase_tick_system),
        ),
    );

    let tribulator = app
        .world_mut()
        .spawn((
            Position::new([0.0, 66.0, 0.0]),
            Cultivation {
                realm: Realm::Spirit,
                qi_current: 200.0,
                qi_max: 210.0,
                ..Default::default()
            },
            Wounds {
                health_current: 100.0,
                health_max: 100.0,
                entries: Vec::new(),
            },
            Lifecycle {
                character_id: "offline:Azure".to_string(),
                ..Default::default()
            },
            TribulationState {
                kind: TribulationKind::DuXu,
                phase: TribulationPhase::Wave(3),
                epicenter: [0.0, 66.0, 0.0],
                wave_current: 3,
                waves_total: 5,
                started_tick: 0,
                phase_started_tick: 1800,
                next_wave_tick: 2100,
                participants: vec!["offline:Azure".to_string()],
                failed: false,
            },
        ))
        .id();

    app.update();

    let state = app
        .world()
        .get::<TribulationState>(tribulator)
        .expect("tribulation should remain active");
    assert_eq!(state.phase, TribulationPhase::HeartDemon);
    assert_eq!(state.wave_current, 3);
    let wounds = app
        .world()
        .get::<Wounds>(tribulator)
        .expect("wounds should remain attached");
    assert_eq!(wounds.health_current, 100.0);
    assert!(wounds.entries.is_empty());
    let events = app.world().resource::<Events<TribulationWaveCleared>>();
    let emitted: Vec<_> = events.get_reader().read(events).cloned().collect();
    assert_eq!(emitted.len(), 1);
    assert_eq!(emitted[0].wave, 4);
}
#[test]
fn pregen_offer_inserts_heart_demon_after_chain_lightning_without_consuming_wave() {
    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 1500 });
    app.add_event::<TribulationLocked>();
    app.add_event::<TribulationWaveCleared>();
    app.add_systems(Update, tribulation_phase_tick_system);

    let entity = app
        .world_mut()
        .spawn((
            TribulationState {
                kind: TribulationKind::DuXu,
                phase: TribulationPhase::Wave(2),
                epicenter: [0.0, 66.0, 0.0],
                wave_current: 2,
                waves_total: 5,
                started_tick: 100,
                phase_started_tick: 1200,
                next_wave_tick: 1500,
                participants: vec!["offline:Azure".to_string()],
                failed: false,
            },
            PendingHeartDemonOffer {
                trigger_id: String::new(),
                payload: HeartDemonOfferV1 {
                    offer_id: "heart-demon-pregen".to_string(),
                    trigger_id: String::new(),
                    trigger_label: "心魔照见".to_string(),
                    realm_label: "渡虚劫 · 心魔".to_string(),
                    composure: 0.7,
                    quota_remaining: 1,
                    quota_total: 1,
                    expires_at_ms: 1,
                    choices: Vec::new(),
                },
            },
        ))
        .id();
    let trigger_id = format!("heart_demon:{}:100", entity.index());
    {
        let mut offer = app
            .world_mut()
            .get_mut::<PendingHeartDemonOffer>(entity)
            .expect("pregen offer should attach");
        offer.trigger_id = trigger_id.clone();
        offer.payload.trigger_id = trigger_id;
    }

    app.update();

    let state = app
        .world()
        .get::<TribulationState>(entity)
        .expect("tribulation should remain active");
    assert_eq!(state.phase, TribulationPhase::HeartDemon);
    assert_eq!(state.wave_current, 2);
    let events = app.world().resource::<Events<TribulationWaveCleared>>();
    let emitted: Vec<_> = events.get_reader().read(events).cloned().collect();
    assert_eq!(emitted.len(), 1);
    assert_eq!(emitted[0].wave, 2);
}
#[test]
fn heart_demon_still_falls_back_to_fourth_slot_when_pregen_is_absent() {
    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 2100 });
    app.add_event::<TribulationLocked>();
    app.add_event::<TribulationWaveCleared>();
    app.add_systems(Update, tribulation_phase_tick_system);
    let entity = app
        .world_mut()
        .spawn(TribulationState {
            kind: TribulationKind::DuXu,
            phase: TribulationPhase::Wave(3),
            epicenter: [0.0, 66.0, 0.0],
            wave_current: 3,
            waves_total: 5,
            started_tick: 0,
            phase_started_tick: 1800,
            next_wave_tick: 2100,
            participants: vec!["offline:Azure".to_string()],
            failed: false,
        })
        .id();

    app.update();

    let state = app
        .world()
        .get::<TribulationState>(entity)
        .expect("tribulation should remain active");
    assert_eq!(state.phase, TribulationPhase::HeartDemon);
    assert_eq!(state.wave_current, 3);
    let events = app.world().resource::<Events<TribulationWaveCleared>>();
    let emitted: Vec<_> = events.get_reader().read(events).cloned().collect();
    assert_eq!(emitted.len(), 1);
    assert_eq!(emitted[0].wave, 4);
}
#[test]
fn resolved_early_heart_demon_continues_next_combat_wave() {
    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 1810 });
    app.add_event::<TribulationLocked>();
    app.add_event::<TribulationWaveCleared>();
    app.add_systems(Update, tribulation_phase_tick_system);
    let entity = app
        .world_mut()
        .spawn((
            TribulationState {
                kind: TribulationKind::DuXu,
                phase: TribulationPhase::HeartDemon,
                epicenter: [0.0, 66.0, 0.0],
                wave_current: 2,
                waves_total: 5,
                started_tick: 100,
                phase_started_tick: 1500,
                next_wave_tick: 1800,
                participants: vec!["offline:Azure".to_string()],
                failed: false,
            },
            HeartDemonResolution {
                outcome: HeartDemonOutcome::Steadfast,
                choice_idx: Some(0),
                tick: 1510,
                next_wave_multiplier: 1.0,
            },
        ))
        .id();

    app.update();

    let state = app
        .world()
        .get::<TribulationState>(entity)
        .expect("tribulation should remain active");
    assert_eq!(state.phase, TribulationPhase::Wave(3));
    assert_eq!(state.wave_current, 2);
    let events = app.world().resource::<Events<TribulationWaveCleared>>();
    let emitted: Vec<_> = events.get_reader().read(events).cloned().collect();
    assert_eq!(emitted.len(), 1);
    assert_eq!(emitted[0].wave, 3);
}
#[test]
fn unresolved_heart_demon_waits_without_advancing_to_kaitian_wave() {
    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 2400 });
    app.add_event::<TribulationLocked>();
    app.add_event::<TribulationWaveCleared>();
    app.add_systems(Update, tribulation_phase_tick_system);
    let entity = app
        .world_mut()
        .spawn(TribulationState {
            kind: TribulationKind::DuXu,
            phase: TribulationPhase::HeartDemon,
            epicenter: [0.0, 66.0, 0.0],
            wave_current: 4,
            waves_total: 5,
            started_tick: 0,
            phase_started_tick: 2100,
            next_wave_tick: 2400,
            participants: vec!["offline:Azure".to_string()],
            failed: false,
        })
        .id();

    app.update();

    let state = app
        .world()
        .get::<TribulationState>(entity)
        .expect("tribulation should remain active");
    assert_eq!(state.phase, TribulationPhase::HeartDemon);
    assert_eq!(state.phase_started_tick, 2100);
    let events = app.world().resource::<Events<TribulationWaveCleared>>();
    let emitted: Vec<_> = events.get_reader().read(events).cloned().collect();
    assert!(emitted.is_empty());
}
#[test]
fn restored_fourth_wave_remains_heart_demon() {
    let state = TribulationState::restored(4, 5, 120);

    assert_eq!(state.phase, TribulationPhase::HeartDemon);
    assert_eq!(state.wave_current, 4);
    assert_eq!(state.waves_total, 5);
}
#[test]
fn heart_demon_steadfast_choice_records_and_restores_qi() {
    // Pitfall (a): give the zone positive headroom under the qi_flow 0.0-floor model
    // (gain_from_zone available = max(spirit_qi, 0.0) × QI_ZONE_UNIT_CAPACITY, #1911/#1931
    // 契约：负/零灵域不产出）。spirit_qi = 0.5 → 25 qi 可用 ≥ 20 qi 全额授予，
    // 不触发容量封顶、不拆账。
    let mut app = qi_test_app();
    app.add_event::<HeartDemonChoiceSubmitted>();
    app.add_systems(Update, heart_demon_choice_system);
    let mut zones = ZoneRegistry::fallback();
    zones
        .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
        .expect("fallback zone should exist")
        .spirit_qi = 0.5;
    app.insert_resource(zones);

    // Pitfall (b): entity needs CurrentDimension for zone lookup to succeed.
    let entity = app
        .world_mut()
        .spawn((
            Position::new([0.0, 70.0, 0.0]), // inside spawn zone (y in [64.0, 80.0])
            CurrentDimension(DimensionKind::Overworld),
            Cultivation {
                realm: Realm::Spirit,
                qi_current: 120.0,
                qi_max: 210.0,
                qi_max_frozen: Some(10.0),
                ..Default::default()
            },
            LifeRecord::new("offline:Azure"),
            TribulationState {
                kind: TribulationKind::DuXu,
                phase: TribulationPhase::HeartDemon,
                epicenter: [0.0, 66.0, 0.0],
                wave_current: 4,
                waves_total: 5,
                started_tick: 0,
                phase_started_tick: 2100,
                next_wave_tick: 2400,
                participants: vec!["offline:Azure".to_string()],
                failed: false,
            },
        ))
        .id();

    app.world_mut().send_event(HeartDemonChoiceSubmitted {
        entity,
        choice_idx: Some(0),
        submitted_at_tick: 2110,
    });
    app.update();

    // effective_qi_max = (210.0 - 10.0).max(0.0) = 200.0
    // desired_grant    = (200.0 * 0.10).min((200.0 - 120.0).max(0.0)) = 20.0
    // available        = max(0.5, 0.0) * 50.0 = 25.0 >= 20.0 → actual_grant = 20.0
    // Pitfall (c): use computed grant, not a hardcoded integer.
    let effective_qi_max = 200.0_f64;
    let expected_grant = (effective_qi_max * 0.10).min((effective_qi_max - 120.0).max(0.0));
    let expected_qi_current = 120.0 + expected_grant;
    // Zone spirit_qi starts at 0.5; debit = 20.0 / 50.0 = 0.4 → zone ends at 0.1.
    // (qi_flow 0.0-floor model：授予永不把 zone 拉进负灵域，与 #1911/#1931 契约一致。)
    let expected_zone_spirit_qi = 0.5 - expected_grant / 50.0;

    let cultivation = app
        .world()
        .get::<Cultivation>(entity)
        .expect("cultivation should remain attached");
    assert!(
        (cultivation.qi_current - expected_qi_current).abs() < 1e-9,
        "Steadfast should grant {expected_grant} qi: expected qi_current={expected_qi_current}, got {}",
        cultivation.qi_current
    );

    let zone_spirit_qi = app
        .world()
        .resource::<ZoneRegistry>()
        .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
        .expect("spawn zone should exist")
        .spirit_qi;
    assert!(
        (zone_spirit_qi - expected_zone_spirit_qi).abs() < 1e-9,
        "Zone must be debited by grant/QI_ZONE_UNIT_CAPACITY to preserve conservation: \
         expected spirit_qi={expected_zone_spirit_qi}, got {zone_spirit_qi}"
    );

    let resolution = app
        .world()
        .get::<HeartDemonResolution>(entity)
        .expect("resolution should be recorded");
    assert_eq!(resolution.outcome, HeartDemonOutcome::Steadfast);
    assert_eq!(resolution.choice_idx, Some(0));
    assert_eq!(resolution.tick, 2110);
    assert_eq!(resolution.next_wave_multiplier, 1.0);
    let life = app
        .world()
        .get::<LifeRecord>(entity)
        .expect("life record should remain attached");
    assert!(matches!(
        life.biography.last(),
        Some(BiographyEntry::HeartDemonRecord {
            outcome: HeartDemonOutcome::Steadfast,
            choice_idx: Some(0),
            tick: 2110
        })
    ));
}
#[test]
fn heart_demon_steadfast_no_grant_without_zone() {
    // When the entity has no CurrentDimension the zone lookup fails and the grant is
    // suppressed to zero — no qi from thin air.
    let mut app = qi_test_app();
    app.add_event::<HeartDemonChoiceSubmitted>();
    app.add_systems(Update, heart_demon_choice_system);
    let mut zones = ZoneRegistry::fallback();
    zones
        .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
        .expect("fallback zone should exist")
        .spirit_qi = 0.0;
    app.insert_resource(zones);

    let entity = app
        .world_mut()
        .spawn((
            Position::new([0.0, 70.0, 0.0]),
            // Deliberately omit CurrentDimension so zone lookup returns None.
            Cultivation {
                realm: Realm::Spirit,
                qi_current: 120.0,
                qi_max: 210.0,
                ..Default::default()
            },
            LifeRecord::new("offline:Azure2"),
            TribulationState {
                kind: TribulationKind::DuXu,
                phase: TribulationPhase::HeartDemon,
                epicenter: [0.0, 66.0, 0.0],
                wave_current: 4,
                waves_total: 5,
                started_tick: 0,
                phase_started_tick: 100,
                next_wave_tick: 400,
                participants: vec!["offline:Azure2".to_string()],
                failed: false,
            },
        ))
        .id();

    app.world_mut().send_event(HeartDemonChoiceSubmitted {
        entity,
        choice_idx: Some(0), // Steadfast
        submitted_at_tick: 110,
    });
    app.update();

    let cultivation = app
        .world()
        .get::<Cultivation>(entity)
        .expect("cultivation should remain attached");
    assert_eq!(
        cultivation.qi_current, 120.0,
        "Without CurrentDimension the zone cannot be located; \
         grant must be suppressed to preserve conservation (qi_current should stay at 120.0)"
    );

    // Zone must not be touched either.
    let zone_spirit_qi = app
        .world()
        .resource::<ZoneRegistry>()
        .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
        .expect("spawn zone should exist")
        .spirit_qi;
    assert_eq!(
        zone_spirit_qi, 0.0,
        "Zone spirit_qi must stay at 0.0 when no grant was applied"
    );
}
#[test]
fn heart_demon_steadfast_capped_by_zone_headroom() {
    // Zone is near-depleted (spirit_qi = 0.1 → only 5 qi available above the 0.0 floor).
    // Player wants 10% of 300 = 30 qi but zone can only provide 5.
    // actual_grant must equal the zone debit (no qi created from thin air).
    let mut app = qi_test_app();
    app.add_event::<HeartDemonChoiceSubmitted>();
    app.add_systems(Update, heart_demon_choice_system);
    let mut zones = ZoneRegistry::fallback();
    zones
        .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
        .expect("fallback zone should exist")
        .spirit_qi = 0.1; // only max(0.1, 0.0) × 50 = 5 qi available
    app.insert_resource(zones);

    let entity = app
        .world_mut()
        .spawn((
            Position::new([0.0, 70.0, 0.0]),
            CurrentDimension(DimensionKind::Overworld),
            Cultivation {
                realm: Realm::Spirit,
                qi_current: 0.0,
                qi_max: 300.0,
                ..Default::default()
            },
            LifeRecord::new("offline:Azure3"),
            TribulationState {
                kind: TribulationKind::DuXu,
                phase: TribulationPhase::HeartDemon,
                epicenter: [0.0, 66.0, 0.0],
                wave_current: 4,
                waves_total: 5,
                started_tick: 0,
                phase_started_tick: 100,
                next_wave_tick: 400,
                participants: vec!["offline:Azure3".to_string()],
                failed: false,
            },
        ))
        .id();

    app.world_mut().send_event(HeartDemonChoiceSubmitted {
        entity,
        choice_idx: Some(0), // Steadfast
        submitted_at_tick: 110,
    });
    app.update();

    // Zone had spirit_qi = 0.1; available = max(0.1, 0.0) * 50.0 = 5.0 qi.
    // desired_grant = (300.0 * 0.10).min((300.0 - 0.0).max(0.0)) = 30.0.
    // actual_grant  = 30.0.min(5.0) = 5.0 (capped by zone headroom).
    let zone_spirit_qi_before = 0.1_f64;
    let zone_available = zone_spirit_qi_before.max(0.0) * 50.0; // 5.0
    let desired_grant = (300.0_f64 * 0.10).min((300.0_f64 - 0.0_f64).max(0.0));
    let expected_grant = desired_grant.min(zone_available);
    let expected_qi_current = 0.0 + expected_grant;
    let expected_zone_spirit_qi = zone_spirit_qi_before - expected_grant / 50.0;

    let cultivation = app
        .world()
        .get::<Cultivation>(entity)
        .expect("cultivation should remain attached");
    assert!(
        (cultivation.qi_current - expected_qi_current).abs() < 1e-9,
        "Grant must be capped at zone headroom ({zone_available} qi): \
         expected qi_current={expected_qi_current}, got {}",
        cultivation.qi_current
    );

    let zone_spirit_qi = app
        .world()
        .resource::<ZoneRegistry>()
        .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
        .expect("spawn zone should exist")
        .spirit_qi;
    assert!(
        (zone_spirit_qi - expected_zone_spirit_qi).abs() < 1e-9,
        "Zone debit must equal actual grant / QI_ZONE_UNIT_CAPACITY: \
         expected spirit_qi={expected_zone_spirit_qi}, got {zone_spirit_qi}"
    );

    // Conservation: player gain == zone debit × QI_ZONE_UNIT_CAPACITY.
    let player_gain = cultivation.qi_current - 0.0;
    let zone_debit = (zone_spirit_qi_before - zone_spirit_qi) * 50.0;
    assert!(
        (player_gain - zone_debit).abs() < 1e-9,
        "Conservation: player_gain ({player_gain}) must equal zone_debit ({zone_debit})"
    );
}
#[test]
fn heart_demon_obsession_timeout_credits_penalty_to_zone() {
    let mut app = qi_test_app();
    app.insert_resource(CombatClock {
        tick: 2100 + DUXU_HEART_DEMON_TIMEOUT_TICKS,
    });
    app.add_event::<QiTransfer>();

    // Empty zone so all 30 qi (= 100 × 0.30) fit without overflow (cap = 50).
    let mut zones = ZoneRegistry::fallback();
    zones
        .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
        .expect("fallback spawn zone should exist")
        .spirit_qi = 0.0;
    app.insert_resource(zones);

    app.add_systems(Update, heart_demon_timeout_system);

    let entity = app
        .world_mut()
        .spawn((
            Position::new([0.0, 64.0, 0.0]),
            CurrentDimension(DimensionKind::Overworld),
            Cultivation {
                realm: Realm::Spirit,
                qi_current: 100.0,
                qi_max: 210.0,
                ..Default::default()
            },
            LifeRecord::new("offline:Azure"),
            TribulationState {
                kind: TribulationKind::DuXu,
                phase: TribulationPhase::HeartDemon,
                epicenter: [0.0, 64.0, 0.0],
                wave_current: 4,
                waves_total: 5,
                started_tick: 0,
                phase_started_tick: 2100,
                next_wave_tick: 2400,
                participants: vec!["offline:Azure".to_string()],
                failed: false,
            },
        ))
        .id();

    app.update();

    // qi_current must be reduced by exactly DUXU_HEART_DEMON_OBSESSION_QI_PENALTY_RATIO.
    let cultivation = app
        .world()
        .get::<Cultivation>(entity)
        .expect("cultivation should remain attached");
    let expected_qi = 100.0 * (1.0 - DUXU_HEART_DEMON_OBSESSION_QI_PENALTY_RATIO);
    assert!(
        (cultivation.qi_current - expected_qi).abs() < 1e-9,
        "qi_current after Obsession penalty: expected {expected_qi}, got {}",
        cultivation.qi_current
    );

    // The drained amount (30.0) must have been credited to the spawn zone.
    // zone cap = 50.0, spirit_qi starts at 0.0, so accepted = 30.0 / 50.0 = 0.60.
    let zone_after = app
        .world()
        .resource::<ZoneRegistry>()
        .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
        .expect("fallback spawn zone should exist")
        .spirit_qi;
    let actual_drain = 100.0 - expected_qi; // = 30.0
    let expected_zone = actual_drain / 50.0; // zone cap = 50.0
    assert!(
        (zone_after - expected_zone).abs() < 1e-9,
        "zone spirit_qi should rise by drain/cap: expected {expected_zone}, got {zone_after}"
    );

    // At least one QiTransfer event must have been emitted for the zone credit.
    let transfers: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<QiTransfer>>()
        .drain()
        .collect();
    assert!(
        !transfers.is_empty(),
        "heart_demon_obsession should emit at least one QiTransfer for zone credit"
    );
    assert_eq!(
        transfers[0].to,
        QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME),
        "QiTransfer destination should be the spawn zone"
    );
}
#[test]
fn heart_demon_obsession_choice_credits_penalty_to_zone() {
    let mut app = qi_test_app();
    app.add_event::<HeartDemonChoiceSubmitted>();
    app.add_event::<QiTransfer>();

    // Empty zone so all 30 qi fit without overflow.
    let mut zones = ZoneRegistry::fallback();
    zones
        .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
        .expect("fallback spawn zone should exist")
        .spirit_qi = 0.0;
    app.insert_resource(zones);

    app.add_systems(Update, heart_demon_choice_system);

    let entity = app
        .world_mut()
        .spawn((
            Position::new([0.0, 64.0, 0.0]),
            CurrentDimension(DimensionKind::Overworld),
            Cultivation {
                realm: Realm::Spirit,
                qi_current: 100.0,
                qi_max: 210.0,
                ..Default::default()
            },
            LifeRecord::new("offline:Azure"),
            TribulationState {
                kind: TribulationKind::DuXu,
                phase: TribulationPhase::HeartDemon,
                epicenter: [0.0, 64.0, 0.0],
                wave_current: 4,
                waves_total: 5,
                started_tick: 0,
                phase_started_tick: 2100,
                next_wave_tick: 2400,
                participants: vec!["offline:Azure".to_string()],
                failed: false,
            },
        ))
        .id();

    // choice_idx = Some(1) → Obsession (not Steadfast/NoSolution)
    app.world_mut().send_event(HeartDemonChoiceSubmitted {
        entity,
        choice_idx: Some(1),
        submitted_at_tick: 2110,
    });
    app.update();

    let cultivation = app
        .world()
        .get::<Cultivation>(entity)
        .expect("cultivation should remain attached");
    let expected_qi = 100.0 * (1.0 - DUXU_HEART_DEMON_OBSESSION_QI_PENALTY_RATIO);
    assert!(
        (cultivation.qi_current - expected_qi).abs() < 1e-9,
        "qi_current after Obsession choice: expected {expected_qi}, got {}",
        cultivation.qi_current
    );

    let zone_after = app
        .world()
        .resource::<ZoneRegistry>()
        .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
        .expect("fallback spawn zone should exist")
        .spirit_qi;
    let actual_drain = 100.0 - expected_qi;
    let expected_zone = actual_drain / 50.0;
    assert!(
        (zone_after - expected_zone).abs() < 1e-9,
        "zone spirit_qi should rise by drain/cap: expected {expected_zone}, got {zone_after}"
    );

    let transfers: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<QiTransfer>>()
        .drain()
        .collect();
    assert!(
        !transfers.is_empty(),
        "heart_demon_obsession choice should emit at least one QiTransfer for zone credit"
    );
}
#[test]
fn heart_demon_no_solution_choice_records_without_penalty_or_boost() {
    let mut app = qi_test_app();
    app.add_event::<HeartDemonChoiceSubmitted>();
    app.add_systems(Update, heart_demon_choice_system);
    let entity = app
        .world_mut()
        .spawn((
            Cultivation {
                realm: Realm::Spirit,
                qi_current: 100.0,
                qi_max: 210.0,
                ..Default::default()
            },
            LifeRecord::new("offline:Azure"),
            TribulationState {
                kind: TribulationKind::DuXu,
                phase: TribulationPhase::HeartDemon,
                epicenter: [0.0, 66.0, 0.0],
                wave_current: 4,
                waves_total: 5,
                started_tick: 0,
                phase_started_tick: 2100,
                next_wave_tick: 2400,
                participants: vec!["offline:Azure".to_string()],
                failed: false,
            },
        ))
        .id();

    app.world_mut().send_event(HeartDemonChoiceSubmitted {
        entity,
        choice_idx: Some(2),
        submitted_at_tick: 2115,
    });
    app.update();

    let cultivation = app
        .world()
        .get::<Cultivation>(entity)
        .expect("cultivation should remain attached");
    assert_eq!(cultivation.qi_current, 100.0);
    let resolution = app
        .world()
        .get::<HeartDemonResolution>(entity)
        .expect("resolution should be recorded");
    assert_eq!(resolution.outcome, HeartDemonOutcome::NoSolution);
    assert_eq!(resolution.choice_idx, Some(2));
    assert_eq!(resolution.tick, 2115);
    assert_eq!(resolution.next_wave_multiplier, 1.0);
    let life = app
        .world()
        .get::<LifeRecord>(entity)
        .expect("life record should remain attached");
    assert!(matches!(
        life.biography.last(),
        Some(BiographyEntry::HeartDemonRecord {
            outcome: HeartDemonOutcome::NoSolution,
            choice_idx: Some(2),
            tick: 2115
        })
    ));
}
#[test]
fn heart_demon_decision_dedup_same_update_identical_obsession() {
    // review finding major-4：同 Update 两条 Obsession 决策只允许首条生效——30% 惩罚
    // 只扣一次（500→350），绝不叠成 500→350→245。
    let mut app = qi_test_app();
    app.add_event::<HeartDemonChoiceSubmitted>();
    app.add_event::<QiTransfer>();
    let mut zones = ZoneRegistry::fallback();
    zones
        .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
        .unwrap()
        .spirit_qi = 0.0;
    app.insert_resource(zones);
    app.add_systems(Update, heart_demon_choice_system);
    let entity = app
        .world_mut()
        .spawn((
            Position::new([0.0, 64.0, 0.0]),
            CurrentDimension(DimensionKind::Overworld),
            Cultivation {
                realm: Realm::Spirit,
                qi_current: 500.0,
                qi_max: 500.0,
                ..Default::default()
            },
            LifeRecord::new("offline:Azure"),
            TribulationState {
                kind: TribulationKind::DuXu,
                phase: TribulationPhase::HeartDemon,
                epicenter: [0.0, 64.0, 0.0],
                wave_current: 4,
                waves_total: 5,
                started_tick: 0,
                phase_started_tick: 2100,
                next_wave_tick: 2400,
                participants: vec!["offline:Azure".to_string()],
                failed: false,
            },
        ))
        .id();

    // 同 tick 发两条 Obsession 决策（修复前两条都因 deferred Commands 不可见而各自生效）。
    app.world_mut().send_event(HeartDemonChoiceSubmitted {
        entity,
        choice_idx: Some(1),
        submitted_at_tick: 2110,
    });
    app.world_mut().send_event(HeartDemonChoiceSubmitted {
        entity,
        choice_idx: Some(1),
        submitted_at_tick: 2111,
    });
    app.update();

    let cultivation = app.world().get::<Cultivation>(entity).unwrap();
    let expected_qi = 500.0 * (1.0 - DUXU_HEART_DEMON_OBSESSION_QI_PENALTY_RATIO);
    assert!(
        (cultivation.qi_current - expected_qi).abs() < 1e-9,
        "同 Update 两条 Obsession 决策只扣一次 30%：期望 qi_current={expected_qi}，实际 {}",
        cultivation.qi_current
    );
    let resolution = app
        .world()
        .get::<HeartDemonResolution>(entity)
        .expect("resolution should be recorded");
    assert_eq!(resolution.outcome, HeartDemonOutcome::Obsession);
    assert_eq!(resolution.choice_idx, Some(1));
    // 只记一条 biography。
    let life = app.world().get::<LifeRecord>(entity).unwrap();
    let records: Vec<_> = life
        .biography
        .iter()
        .filter(|e| matches!(e, BiographyEntry::HeartDemonRecord { .. }))
        .collect();
    assert_eq!(
        records.len(),
        1,
        "重复决策只应落一条 HeartDemonRecord biography"
    );
}
#[test]
fn heart_demon_decision_dedup_same_update_mixed_first_wins() {
    // review finding major-4：同 Update 混合包（先 Steadfast 后 Obsession）只保留
    // **首个**（Steadfast 回真元），后续 Obsession 不得再扣——修复前的"最后一条取胜"
    // 会同时 grant 又 drain、multipler 交代不清。
    let mut app = qi_test_app();
    app.add_event::<HeartDemonChoiceSubmitted>();
    app.add_event::<QiTransfer>();
    let mut zones = ZoneRegistry::fallback();
    zones
        .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
        .unwrap()
        .spirit_qi = 0.5; // 足够授予
    app.insert_resource(zones);
    app.add_systems(Update, heart_demon_choice_system);
    let entity = app
        .world_mut()
        .spawn((
            Position::new([0.0, 70.0, 0.0]),
            CurrentDimension(DimensionKind::Overworld),
            Cultivation {
                realm: Realm::Spirit,
                qi_current: 120.0,
                qi_max: 210.0,
                qi_max_frozen: Some(10.0),
                ..Default::default()
            },
            LifeRecord::new("offline:Azure"),
            TribulationState {
                kind: TribulationKind::DuXu,
                phase: TribulationPhase::HeartDemon,
                epicenter: [0.0, 66.0, 0.0],
                wave_current: 4,
                waves_total: 5,
                started_tick: 0,
                phase_started_tick: 2100,
                next_wave_tick: 2400,
                participants: vec!["offline:Azure".to_string()],
                failed: false,
            },
        ))
        .id();

    app.world_mut().send_event(HeartDemonChoiceSubmitted {
        entity,
        choice_idx: Some(0), // Steadfast（首个）
        submitted_at_tick: 2110,
    });
    app.world_mut().send_event(HeartDemonChoiceSubmitted {
        entity,
        choice_idx: Some(1), // Obsession（同 Update 后续，应被拒绝）
        submitted_at_tick: 2111,
    });
    app.update();

    // effective_qi_max = 200；desired_grant = min(20, 80) = 20 → qi 120 + 20 = 140；
    // 后续 Obsession 30% 不得生效（否则再 drain 42 → 98）。
    let cultivation = app.world().get::<Cultivation>(entity).unwrap();
    assert!(
        (cultivation.qi_current - 140.0).abs() < 1e-9,
        "首个 Steadfast 授予 20（120→140），后续 Obsession 不得再扣：实际 {}",
        cultivation.qi_current
    );
    let resolution = app
        .world()
        .get::<HeartDemonResolution>(entity)
        .expect("resolution should be recorded");
    assert_eq!(resolution.outcome, HeartDemonOutcome::Steadfast);
    assert_eq!(resolution.choice_idx, Some(0));
    assert_eq!(resolution.next_wave_multiplier, 1.0);
}
#[test]
fn heart_demon_decision_dedup_later_delivery_second_ignored() {
    // review finding major-4：跨 Update 的重复决策——首个 Update 生效并落解析组件；
    // 下一 Update 再发的包被已有解析拒绝（首条保留）。
    let mut app = qi_test_app();
    app.add_event::<HeartDemonChoiceSubmitted>();
    app.add_event::<QiTransfer>();
    let mut zones = ZoneRegistry::fallback();
    zones
        .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
        .unwrap()
        .spirit_qi = 0.0;
    app.insert_resource(zones);
    app.add_systems(Update, heart_demon_choice_system);
    let entity = app
        .world_mut()
        .spawn((
            Position::new([0.0, 64.0, 0.0]),
            CurrentDimension(DimensionKind::Overworld),
            Cultivation {
                realm: Realm::Spirit,
                qi_current: 500.0,
                qi_max: 500.0,
                ..Default::default()
            },
            LifeRecord::new("offline:Azure"),
            TribulationState {
                kind: TribulationKind::DuXu,
                phase: TribulationPhase::HeartDemon,
                epicenter: [0.0, 64.0, 0.0],
                wave_current: 4,
                waves_total: 5,
                started_tick: 0,
                phase_started_tick: 2100,
                next_wave_tick: 2400,
                participants: vec!["offline:Azure".to_string()],
                failed: false,
            },
        ))
        .id();

    // Update 1：首条 Obsession 生效。
    app.world_mut().send_event(HeartDemonChoiceSubmitted {
        entity,
        choice_idx: Some(1),
        submitted_at_tick: 2110,
    });
    app.update();
    let after_first = app.world().get::<Cultivation>(entity).unwrap().qi_current;
    assert!(
        (after_first - 500.0 * (1.0 - DUXU_HEART_DEMON_OBSESSION_QI_PENALTY_RATIO)).abs() < 1e-9
    );

    // Update 2：重复 Obsession 包（跨 Update，解析组件已存在）→ 不得再扣。
    app.world_mut().send_event(HeartDemonChoiceSubmitted {
        entity,
        choice_idx: Some(1),
        submitted_at_tick: 2115,
    });
    app.update();
    let after_second = app.world().get::<Cultivation>(entity).unwrap().qi_current;
    assert!(
        (after_second - after_first).abs() < 1e-9,
        "跨 Update 重复决策不得二次扣减：首条后 {after_first}，次条后 {after_second}"
    );
    let resolution = app.world().get::<HeartDemonResolution>(entity).unwrap();
    assert_eq!(resolution.choice_idx, Some(1));
    assert_eq!(resolution.tick, 2110);
}
#[test]
fn publish_lock_event_to_tribulation_channel() {
    use bong_server::network::audio_event_emit::PlaySoundRecipeRequest;
    let mut app = qi_test_app();
    let (tx_outbound, rx_outbound) = crossbeam_channel::unbounded();
    let (_tx_inbound, rx_inbound) = crossbeam_channel::unbounded();
    app.insert_resource(RedisBridgeResource {
        tx_outbound,
        rx_inbound,
    });
    app.add_event::<TribulationAnnounce>();
    app.add_event::<TribulationLocked>();
    app.add_event::<TribulationWaveCleared>();
    app.add_event::<TribulationSettled>();
    app.add_event::<JueBiTriggeredEvent>();
    app.add_event::<AscensionQuotaOpened>();
    app.add_event::<PlaySoundRecipeRequest>();
    app.add_systems(Update, publish_tribulation_events);

    app.world_mut()
        .resource_mut::<Events<TribulationLocked>>()
        .send(TribulationLocked {
            entity: Entity::PLACEHOLDER,
            char_id: "offline:Azure".to_string(),
            actor_name: "Azure".to_string(),
            epicenter: [12.0, 66.0, -8.0],
            waves_total: 3,
        });

    app.update();

    let outbound = rx_outbound
        .try_recv()
        .expect("lock event should publish to redis bridge");
    match outbound {
        RedisOutbound::TribulationEvent(payload) => {
            assert_eq!(payload.phase, TribulationPhaseV1::Lock);
            assert_eq!(payload.char_id.as_deref(), Some("offline:Azure"));
            assert_eq!(payload.actor_name.as_deref(), Some("Azure"));
            assert_eq!(payload.epicenter, Some([12.0, 66.0, -8.0]));
            assert_eq!(payload.wave_total, Some(3));
        }
        other => panic!("unexpected outbound payload: {other:?}"),
    }
}
#[test]
fn publish_wave_event_keeps_tribulator_identity() {
    use bong_server::network::audio_event_emit::PlaySoundRecipeRequest;
    let mut app = qi_test_app();
    let (tx_outbound, rx_outbound) = crossbeam_channel::unbounded();
    let (_tx_inbound, rx_inbound) = crossbeam_channel::unbounded();
    app.insert_resource(RedisBridgeResource {
        tx_outbound,
        rx_inbound,
    });
    app.add_event::<TribulationAnnounce>();
    app.add_event::<TribulationLocked>();
    app.add_event::<TribulationWaveCleared>();
    app.add_event::<TribulationSettled>();
    app.add_event::<JueBiTriggeredEvent>();
    app.add_event::<AscensionQuotaOpened>();
    app.add_event::<PlaySoundRecipeRequest>();
    app.add_systems(Update, publish_tribulation_events);

    let entity = app
        .world_mut()
        .spawn((
            Lifecycle {
                character_id: "offline:Azure".to_string(),
                ..Default::default()
            },
            Username("Azure".to_string()),
            TribulationState {
                kind: TribulationKind::DuXu,
                phase: TribulationPhase::Wave(2),
                epicenter: [12.0, 66.0, -8.0],
                wave_current: 1,
                waves_total: 5,
                started_tick: 0,
                phase_started_tick: 1200,
                next_wave_tick: 1500,
                participants: vec!["offline:Azure".to_string()],
                failed: false,
            },
        ))
        .id();
    app.world_mut()
        .resource_mut::<Events<TribulationWaveCleared>>()
        .send(TribulationWaveCleared { entity, wave: 2 });

    app.update();

    let outbound = rx_outbound
        .try_recv()
        .expect("wave event should publish to redis bridge");
    match outbound {
        RedisOutbound::TribulationEvent(payload) => {
            assert_eq!(payload.phase, TribulationPhaseV1::Wave { wave: 2 });
            assert_eq!(payload.char_id.as_deref(), Some("offline:Azure"));
            assert_eq!(payload.actor_name.as_deref(), Some("Azure"));
            assert_eq!(payload.epicenter, Some([12.0, 66.0, -8.0]));
            assert_eq!(payload.wave_current, Some(2));
            assert_eq!(payload.wave_total, Some(5));
        }
        other => panic!("unexpected outbound payload: {other:?}"),
    }
}
#[test]
fn publish_settle_event_uses_actor_name() {
    use bong_server::network::audio_event_emit::PlaySoundRecipeRequest;
    let mut app = qi_test_app();
    let (tx_outbound, rx_outbound) = crossbeam_channel::unbounded();
    let (_tx_inbound, rx_inbound) = crossbeam_channel::unbounded();
    app.insert_resource(RedisBridgeResource {
        tx_outbound,
        rx_inbound,
    });
    app.add_event::<TribulationAnnounce>();
    app.add_event::<TribulationLocked>();
    app.add_event::<TribulationWaveCleared>();
    app.add_event::<TribulationSettled>();
    app.add_event::<JueBiTriggeredEvent>();
    app.add_event::<AscensionQuotaOpened>();
    app.add_event::<PlaySoundRecipeRequest>();
    app.add_systems(Update, publish_tribulation_events);

    let entity = app
        .world_mut()
        .spawn((
            Lifecycle {
                character_id: "offline:Azure".to_string(),
                ..Default::default()
            },
            Username("Azure".to_string()),
        ))
        .id();
    app.world_mut()
        .resource_mut::<Events<TribulationSettled>>()
        .send(TribulationSettled {
            entity,
            kind: TribulationKind::DuXu,
            source: None,
            result: DuXuResultV1 {
                char_id: "offline:Azure".to_string(),
                outcome: DuXuOutcomeV1::Ascended,
                killer: None,
                waves_survived: 5,
                reason: None,
            },
        });

    app.update();

    let outbound = rx_outbound
        .try_recv()
        .expect("settle event should publish to redis bridge");
    match outbound {
        RedisOutbound::TribulationEvent(payload) => {
            assert_eq!(payload.phase, TribulationPhaseV1::Settle);
            assert_eq!(payload.char_id.as_deref(), Some("offline:Azure"));
            assert_eq!(payload.actor_name.as_deref(), Some("Azure"));
            assert_eq!(
                payload.result.expect("settle should carry result").outcome,
                DuXuOutcomeV1::Ascended
            );
        }
        other => panic!("unexpected outbound payload: {other:?}"),
    }
}
#[test]
fn publish_ascension_quota_open_event_to_tribulation_channel() {
    use bong_server::network::audio_event_emit::PlaySoundRecipeRequest;
    let mut app = qi_test_app();
    let (tx_outbound, rx_outbound) = crossbeam_channel::unbounded();
    let (_tx_inbound, rx_inbound) = crossbeam_channel::unbounded();
    app.insert_resource(RedisBridgeResource {
        tx_outbound,
        rx_inbound,
    });
    app.add_event::<TribulationAnnounce>();
    app.add_event::<TribulationLocked>();
    app.add_event::<TribulationWaveCleared>();
    app.add_event::<TribulationSettled>();
    app.add_event::<JueBiTriggeredEvent>();
    app.add_event::<AscensionQuotaOpened>();
    app.add_event::<PlaySoundRecipeRequest>();
    app.add_systems(Update, publish_tribulation_events);

    app.world_mut()
        .resource_mut::<Events<AscensionQuotaOpened>>()
        .send(AscensionQuotaOpened { occupied_slots: 1 });

    app.update();

    let outbound = rx_outbound
        .try_recv()
        .expect("quota open event should publish to redis bridge");
    match outbound {
        RedisOutbound::TribulationEvent(payload) => {
            assert_eq!(
                payload.kind,
                bong_server::schema::tribulation::TribulationKindV1::AscensionQuotaOpen
            );
            assert_eq!(payload.phase, TribulationPhaseV1::Settle);
            assert_eq!(payload.occupied_slots, Some(1));
        }
        other => panic!("unexpected outbound payload: {other:?}"),
    }
}
#[test]
fn ascension_quota_open_emits_broadcast_audio() {
    use bong_server::network::audio_event_emit::PlaySoundRecipeRequest;
    use bong_server::network::halfstep_rechallenge_emit::HALFSTEP_QUOTA_RELEASE_BROADCAST_AUDIO_RECIPE;

    let mut app = qi_test_app();
    let (tx_outbound, _rx_outbound) = crossbeam_channel::unbounded();
    let (_tx_inbound, rx_inbound) = crossbeam_channel::unbounded();
    app.insert_resource(RedisBridgeResource {
        tx_outbound,
        rx_inbound,
    });
    app.add_event::<TribulationAnnounce>();
    app.add_event::<TribulationLocked>();
    app.add_event::<TribulationWaveCleared>();
    app.add_event::<TribulationSettled>();
    app.add_event::<JueBiTriggeredEvent>();
    app.add_event::<AscensionQuotaOpened>();
    app.add_event::<PlaySoundRecipeRequest>();
    app.add_systems(Update, publish_tribulation_events);

    app.world_mut()
        .resource_mut::<Events<AscensionQuotaOpened>>()
        .send(AscensionQuotaOpened { occupied_slots: 0 });

    app.update();

    let audio_events = app.world().resource::<Events<PlaySoundRecipeRequest>>();
    let mut reader = bevy_ecs::event::ManualEventReader::default();
    let requests: Vec<_> = reader.read(audio_events).collect();
    let broadcast_count = requests
        .iter()
        .filter(|r| r.recipe_id == HALFSTEP_QUOTA_RELEASE_BROADCAST_AUDIO_RECIPE)
        .count();
    assert_eq!(
        broadcast_count, 1,
        "AscensionQuotaOpened 必须发出 1 条 {} 广播音效请求，实际 {}；\
         该 recipe 在 audio registry 已注册，不 emit 是死资产",
        HALFSTEP_QUOTA_RELEASE_BROADCAST_AUDIO_RECIPE, broadcast_count
    );
    // 确认是 All recipient（全服广播）
    let req = requests
        .iter()
        .find(|r| r.recipe_id == HALFSTEP_QUOTA_RELEASE_BROADCAST_AUDIO_RECIPE)
        .unwrap();
    assert!(
        matches!(
            req.recipient,
            bong_server::network::audio_event_emit::AudioRecipient::All
        ),
        "名额空出广播音效必须 recipient=All（全服），实际 {:?}",
        req.recipient
    );
}
#[test]
fn void_quota_limit_uses_world_qi_budget_floor() {
    // 本测试锁的是 compute_void_quota_limit 纯数学 floor-division 边界行为，
    // 与生产 DEFAULT_VOID_QUOTA_K 的具体尺度无关（plan-zone-qi-economy-v1 P0
    // §8.1 决议 #2 之后 DEFAULT_VOID_QUOTA_K 随 DEFAULT_SPIRIT_QI_TOTAL 等比例
    // 缩放，若此处仍绑死生产常量，下方 49.999/50.0/99.999/100.0 边界值会随之
    // 失效）。用测试本地固定 k=50.0 保持边界值语义不变、且与生产常量解耦。
    let k = 50.0;
    assert_eq!(compute_void_quota_limit(0.0, k), 0);
    assert_eq!(compute_void_quota_limit(49.999, k), 0);
    assert_eq!(compute_void_quota_limit(50.0, k), 1);
    assert_eq!(compute_void_quota_limit(99.999, k), 1);
    assert_eq!(compute_void_quota_limit(100.0, k), 2);
    assert_eq!(compute_void_quota_limit(-1.0, k), 0);
    assert_eq!(compute_void_quota_limit(f64::NAN, k), 0);
    assert_eq!(compute_void_quota_limit(100.0, 0.0), 0);
}
#[test]
fn check_void_quota_allows_zero_and_reports_availability() {
    let config = VoidQuotaConfig { quota_k: 50.0 };
    let low_budget = WorldQiBudget::from_total(100.0);
    let mut depleted = low_budget;
    depleted.current_total = 49.0;

    let depleted_check = check_void_quota(0, &depleted, &config);
    assert_eq!(depleted_check.quota_limit, 0);
    assert_eq!(depleted_check.available_slots, 0);
    assert!(depleted_check.exceeded);

    let check = check_void_quota(1, &low_budget, &config);
    assert_eq!(check.quota_limit, 2);
    assert_eq!(check.available_slots, 1);
    assert!(!check.exceeded);

    let full = check_void_quota(2, &low_budget, &config);
    assert_eq!(full.available_slots, 0);
    assert!(full.exceeded);
}
#[test]
fn phase7_balance_interception_window_matches_lock_and_heart_demon_timing() {
    let windows = [DUXU_WAVE_COOLDOWN_TICKS, DUXU_HEART_DEMON_TIMEOUT_TICKS];
    let radii = [
        DUXU_LOCK_RADIUS_FINAL,
        DUXU_LOCK_RADIUS_HARD,
        TRIBULATION_DANGER_RADIUS,
    ];

    assert_eq!(windows, [15 * 20, 30 * 20]);
    assert_eq!(radii, [10.0, 20.0, 100.0]);
    assert!(radii.windows(2).all(|pair| pair[0] <= pair[1]));
}
#[test]
fn lock_expiry_starts_first_wave_and_schedules_cooldown() {
    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 900 });
    app.add_event::<TribulationLocked>();
    app.add_event::<TribulationWaveCleared>();
    app.add_systems(Update, tribulation_phase_tick_system);

    let entity = app
        .world_mut()
        .spawn(TribulationState {
            kind: TribulationKind::DuXu,
            phase: TribulationPhase::Lock,
            epicenter: [0.0, 66.0, 0.0],
            wave_current: 0,
            waves_total: 3,
            started_tick: 0,
            phase_started_tick: 300,
            next_wave_tick: 0,
            participants: vec!["offline:Azure".to_string()],
            failed: false,
        })
        .id();

    app.update();

    let state = app
        .world()
        .get::<TribulationState>(entity)
        .expect("tribulation should remain active");
    assert_eq!(state.phase, TribulationPhase::Wave(1));
    assert_eq!(state.phase_started_tick, 900);
    assert_eq!(state.next_wave_tick, 900 + DUXU_WAVE_COOLDOWN_TICKS);
    let events = app.world().resource::<Events<TribulationWaveCleared>>();
    let emitted: Vec<_> = events.get_reader().read(events).cloned().collect();
    assert_eq!(emitted.len(), 1);
    assert_eq!(emitted[0].entity, entity);
    assert_eq!(emitted[0].wave, 1);
}
#[test]
fn wave_cooldown_starts_next_wave_without_reusing_first_wave_phase() {
    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 1200 });
    app.add_event::<TribulationLocked>();
    app.add_event::<TribulationWaveCleared>();
    app.add_systems(Update, tribulation_phase_tick_system);

    let entity = app
        .world_mut()
        .spawn(TribulationState {
            kind: TribulationKind::DuXu,
            phase: TribulationPhase::Wave(1),
            epicenter: [0.0, 66.0, 0.0],
            wave_current: 1,
            waves_total: 3,
            started_tick: 0,
            phase_started_tick: 900,
            next_wave_tick: 1200,
            participants: vec!["offline:Azure".to_string()],
            failed: false,
        })
        .id();

    app.update();

    let state = app
        .world()
        .get::<TribulationState>(entity)
        .expect("tribulation should remain active");
    assert_eq!(state.phase, TribulationPhase::Wave(2));
    assert_eq!(state.phase_started_tick, 1200);
    assert_eq!(state.next_wave_tick, 1200 + DUXU_WAVE_COOLDOWN_TICKS);
    let events = app.world().resource::<Events<TribulationWaveCleared>>();
    let emitted: Vec<_> = events.get_reader().read(events).cloned().collect();
    assert_eq!(emitted.len(), 1);
    assert_eq!(emitted[0].wave, 2);
}
#[test]
fn tribulation_aoe_ignores_targets_in_other_dimension() {
    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 1200 });
    app.add_event::<TribulationFailed>();
    app.add_event::<DeathEvent>();
    app.add_systems(Update, tribulation_aoe_system);

    app.world_mut().spawn((
        CurrentDimension(DimensionKind::Overworld),
        TribulationState {
            kind: TribulationKind::DuXu,
            phase: TribulationPhase::Wave(2),
            epicenter: [0.0, 66.0, 0.0],
            wave_current: 2,
            waves_total: 3,
            started_tick: 0,
            phase_started_tick: 1200,
            next_wave_tick: 1500,
            participants: vec!["offline:Azure".to_string()],
            failed: false,
        },
    ));
    let target = app
        .world_mut()
        .spawn((
            Position::new([8.0, 66.0, 0.0]),
            CurrentDimension(DimensionKind::Tsy),
            Cultivation {
                realm: Realm::Spirit,
                qi_current: 100.0,
                qi_max: 210.0,
                ..Default::default()
            },
            Wounds {
                health_current: 100.0,
                health_max: 100.0,
                entries: Vec::new(),
            },
            Lifecycle {
                character_id: "offline:Spectator".to_string(),
                ..Default::default()
            },
            LifeRecord::new("offline:Spectator"),
        ))
        .id();

    app.update();

    let wounds = app
        .world()
        .get::<Wounds>(target)
        .expect("wounds should remain attached");
    assert_eq!(wounds.health_current, 100.0);
    assert!(wounds.entries.is_empty());
    let cultivation = app
        .world()
        .get::<Cultivation>(target)
        .expect("cultivation should remain attached");
    assert_eq!(cultivation.qi_current, 100.0);
}
#[test]
fn kaitian_lightning_fails_tribulator_without_full_health() {
    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 2100 });
    app.add_event::<TribulationFailed>();
    app.add_event::<DeathEvent>();
    app.add_systems(Update, tribulation_aoe_system);

    let entity = app
        .world_mut()
        .spawn((
            Position::new([0.0, 66.0, 0.0]),
            Cultivation {
                realm: Realm::Spirit,
                qi_current: 210.0,
                qi_max: 210.0,
                ..Default::default()
            },
            Wounds {
                health_current: 99.0,
                health_max: 100.0,
                entries: Vec::new(),
            },
            Lifecycle {
                character_id: "offline:Azure".to_string(),
                ..Default::default()
            },
            TribulationState {
                kind: TribulationKind::DuXu,
                phase: TribulationPhase::Wave(5),
                epicenter: [0.0, 66.0, 0.0],
                wave_current: 5,
                waves_total: 5,
                started_tick: 0,
                phase_started_tick: 2100,
                next_wave_tick: 2400,
                participants: vec!["offline:Azure".to_string()],
                failed: false,
            },
        ))
        .id();

    app.update();

    let failures = app.world().resource::<Events<TribulationFailed>>();
    let emitted: Vec<_> = failures.get_reader().read(failures).cloned().collect();
    assert_eq!(emitted.len(), 1);
    assert_eq!(emitted[0].entity, entity);
    assert_eq!(emitted[0].wave, 5);
    let wounds = app
        .world()
        .get::<Wounds>(entity)
        .expect("wounds should remain attached");
    assert_eq!(wounds.health_current, 99.0);
    assert!(wounds.entries.is_empty());
}
#[test]
fn kaitian_lightning_fails_tribulator_without_full_available_qi() {
    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 2100 });
    app.add_event::<TribulationFailed>();
    app.add_event::<DeathEvent>();
    app.add_systems(Update, tribulation_aoe_system);

    let entity = app
        .world_mut()
        .spawn((
            Position::new([0.0, 66.0, 0.0]),
            Cultivation {
                realm: Realm::Spirit,
                qi_current: 189.0,
                qi_max: 210.0,
                qi_max_frozen: Some(20.0),
                ..Default::default()
            },
            Wounds {
                health_current: 100.0,
                health_max: 100.0,
                entries: Vec::new(),
            },
            Lifecycle {
                character_id: "offline:Azure".to_string(),
                ..Default::default()
            },
            TribulationState {
                kind: TribulationKind::DuXu,
                phase: TribulationPhase::Wave(5),
                epicenter: [0.0, 66.0, 0.0],
                wave_current: 5,
                waves_total: 5,
                started_tick: 0,
                phase_started_tick: 2100,
                next_wave_tick: 2400,
                participants: vec!["offline:Azure".to_string()],
                failed: false,
            },
        ))
        .id();

    app.update();

    let failures = app.world().resource::<Events<TribulationFailed>>();
    let emitted: Vec<_> = failures.get_reader().read(failures).cloned().collect();
    assert_eq!(emitted.len(), 1);
    assert_eq!(emitted[0].entity, entity);
    assert_eq!(emitted[0].wave, 5);
    let cultivation = app
        .world()
        .get::<Cultivation>(entity)
        .expect("cultivation should remain attached");
    assert_eq!(cultivation.qi_current, 189.0);
}
#[test]
fn kaitian_lightning_hits_normally_when_tribulator_has_full_resources() {
    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 2100 });
    app.add_event::<TribulationFailed>();
    app.add_event::<DeathEvent>();
    app.add_systems(Update, tribulation_aoe_system);

    let entity = app
        .world_mut()
        .spawn((
            Position::new([0.0, 66.0, 0.0]),
            Cultivation {
                realm: Realm::Spirit,
                qi_current: 190.0,
                qi_max: 210.0,
                qi_max_frozen: Some(20.0),
                ..Default::default()
            },
            Wounds {
                health_current: 200.0,
                health_max: 200.0,
                entries: Vec::new(),
            },
            Lifecycle {
                character_id: "offline:Azure".to_string(),
                ..Default::default()
            },
            LifeRecord::new("offline:Azure"),
            TribulationState {
                kind: TribulationKind::DuXu,
                phase: TribulationPhase::Wave(5),
                epicenter: [0.0, 66.0, 0.0],
                wave_current: 5,
                waves_total: 5,
                started_tick: 0,
                phase_started_tick: 2100,
                next_wave_tick: 2400,
                participants: vec!["offline:Azure".to_string()],
                failed: false,
            },
        ))
        .id();

    app.update();

    assert_eq!(app.world().resource::<Events<TribulationFailed>>().len(), 0);
    let wounds = app
        .world()
        .get::<Wounds>(entity)
        .expect("wounds should remain attached");
    assert_eq!(wounds.health_current, 200.0 - DUXU_AOE_DAMAGE_BASE * 5.0);
    assert_eq!(wounds.entries.len(), 1);
    let cultivation = app
        .world()
        .get::<Cultivation>(entity)
        .expect("cultivation should remain attached");
    assert_eq!(cultivation.qi_current, 190.0 - DUXU_QI_DRAIN_BASE * 5.0);
}
#[test]
fn void_quota_exceeded_start_marks_du_xu_for_juebi_instead_of_terminal_death() {
    let mut app = qi_test_app();
    let (settings, root) = persistence_settings("void-quota-exceeded-start");
    let char_id = "offline:Azure";
    let mut depleted_budget = WorldQiBudget::from_total(100.0);
    depleted_budget.current_total = 0.0;

    app.insert_resource(settings.clone());
    app.insert_resource(depleted_budget);
    app.insert_resource(VoidQuotaConfig { quota_k: 50.0 });
    app.add_event::<InitiateXuhuaTribulation>();
    app.add_event::<TribulationAnnounce>();
    app.add_event::<TribulationSettled>();
    app.add_event::<CultivationDeathTrigger>();
    app.add_event::<VfxEventRequest>();
    app.add_systems(Update, start_tribulation_system);

    let entity = app
        .world_mut()
        .spawn((
            Cultivation {
                realm: Realm::Spirit,
                qi_current: 210.0,
                qi_max: 210.0,
                ..Default::default()
            },
            all_meridians_open(),
            Wounds::default(),
            Lifecycle {
                character_id: char_id.to_string(),
                ..Default::default()
            },
            LifeRecord::new(char_id),
            DeathRegistry {
                char_id: char_id.to_string(),
                death_count: 0,
                last_death_tick: None,
                prev_death_tick: None,
                last_death_zone: None,
            },
            LifespanComponent {
                born_at_tick: 0,
                years_lived: 80.0,
                cap_by_realm: LifespanCapTable::SPIRIT,
                offline_pause_tick: None,
            },
            Position::new([0.0, 66.0, 0.0]),
        ))
        .id();
    app.world_mut().send_event(InitiateXuhuaTribulation {
        entity,
        waves_total: 5,
        started_tick: 120,
    });

    app.update();

    let entity_ref = app.world().entity(entity);
    let state = entity_ref
        .get::<TribulationState>()
        .expect("over-quota DuXu should still start and append JueBi at settlement");
    assert_eq!(state.kind, TribulationKind::DuXu);
    assert!(entity_ref.get::<JueBiAfterDuXuQuota>().is_some());

    let death_triggers = app.world().resource::<Events<CultivationDeathTrigger>>();
    let deaths: Vec<_> = death_triggers
        .get_reader()
        .read(death_triggers)
        .cloned()
        .collect();
    assert!(deaths.is_empty());

    let settled = app.world().resource::<Events<TribulationSettled>>();
    let emitted: Vec<_> = settled.get_reader().read(settled).cloned().collect();
    assert!(emitted.is_empty());
    assert!(
        load_active_tribulation(&settings, char_id)
            .expect("active tribulation query should succeed")
            .is_some(),
        "over-quota DuXu should persist until JueBi settlement"
    );
    let quota = load_ascension_quota(&settings).expect("quota load should succeed");
    assert_eq!(quota.occupied_slots, 0);

    let _ = fs::remove_dir_all(root);
}
#[test]
fn juebi_trigger_event_starts_juebi_state_after_delay() {
    let mut app = qi_test_app();
    let (settings, root) = persistence_settings("juebi-trigger-start");
    let char_id = "offline:Azure";
    app.insert_resource(settings.clone());
    app.insert_resource(CombatClock { tick: 40 });
    app.insert_resource(PendingJueBiTriggers::default());
    app.add_event::<JueBiTriggerEvent>();
    app.add_event::<JueBiTriggeredEvent>();
    app.add_systems(
        Update,
        (
            schedule_juebi_triggers_system,
            start_due_juebi_triggers_system.after(schedule_juebi_triggers_system),
        ),
    );
    let entity = app
        .world_mut()
        .spawn((
            Lifecycle {
                character_id: char_id.to_string(),
                ..Default::default()
            },
            Username("Azure".to_string()),
            Position::new([12.0, 66.0, -3.0]),
            Cultivation {
                realm: Realm::Void,
                qi_current: 300.0,
                qi_max: 300.0,
                ..Default::default()
            },
        ))
        .id();
    app.world_mut().send_event(JueBiTriggerEvent {
        entity,
        source: JueBiTriggerSource::VoidActionExplodeZone,
        delay_ticks: 0,
        triggered_at_tick: 40,
        epicenter: None,
    });

    app.update();

    let state = app
        .world()
        .get::<TribulationState>(entity)
        .expect("JueBi trigger should insert active tribulation");
    assert_eq!(state.kind, TribulationKind::JueBi);
    assert_eq!(state.phase, TribulationPhase::Omen);
    assert_eq!(state.epicenter, [12.0, 66.0, -3.0]);
    let events = app.world().resource::<Events<JueBiTriggeredEvent>>();
    assert_eq!(events.len(), 1);
    let active = load_active_tribulation(&settings, char_id)
        .expect("active tribulation query should succeed")
        .expect("JueBi trigger should persist active row");
    assert_eq!(active.kind, "jue_bi");
    assert_eq!(active.epicenter, [12.0, 66.0, -3.0]);

    let _ = fs::remove_dir_all(root);
}
#[test]
fn juebi_pressure_collapse_drains_qi_and_marks_targets() {
    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 100 });
    app.insert_resource(JueBiNullFields::default());
    app.add_event::<DeathEvent>();
    app.add_systems(Update, juebi_phase_effect_system);
    let tribulator = app
        .world_mut()
        .spawn(TribulationState {
            kind: TribulationKind::JueBi,
            phase: TribulationPhase::Wave(1),
            epicenter: [0.0, 64.0, 0.0],
            wave_current: 0,
            waves_total: JUEBI_WAVES_TOTAL,
            started_tick: 0,
            phase_started_tick: 100,
            next_wave_tick: 100 + JUEBI_PHASE_TICKS,
            participants: vec!["offline:Azure".to_string()],
            failed: false,
        })
        .id();
    let target = app
        .world_mut()
        .spawn((
            Position::new([10.0, 64.0, 0.0]),
            Cultivation {
                realm: Realm::Void,
                qi_current: 100.0,
                qi_max: 100.0,
                ..Default::default()
            },
            Wounds::default(),
            Lifecycle {
                character_id: "offline:Azure".to_string(),
                ..Default::default()
            },
            LifeRecord::new("offline:Azure"),
        ))
        .id();
    let _ = tribulator;

    app.update();

    let cultivation = app.world().get::<Cultivation>(target).unwrap();
    assert!(cultivation.qi_current < 100.0);
    assert!(app.world().get::<JueBiPressureCollapse>(target).is_some());
}
#[test]
fn juebi_phase_effect_clears_stale_phase_markers() {
    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 200 });
    app.insert_resource(JueBiNullFields::default());
    app.add_event::<DeathEvent>();
    app.add_systems(Update, juebi_phase_effect_system);
    app.world_mut().spawn(TribulationState {
        kind: TribulationKind::JueBi,
        phase: TribulationPhase::Wave(2),
        epicenter: [0.0, 64.0, 0.0],
        wave_current: 1,
        waves_total: JUEBI_WAVES_TOTAL,
        started_tick: 0,
        phase_started_tick: 200,
        next_wave_tick: 200 + JUEBI_PHASE_TICKS,
        participants: vec!["offline:Azure".to_string()],
        failed: false,
    });
    let target = app
        .world_mut()
        .spawn((
            Position::new([10.0, 64.0, 0.0]),
            Cultivation {
                realm: Realm::Void,
                qi_current: 100.0,
                qi_max: 100.0,
                ..Default::default()
            },
            Wounds::default(),
            Lifecycle {
                character_id: "offline:Azure".to_string(),
                ..Default::default()
            },
            JueBiPressureCollapse {
                epicenter: BlockPos::new(0, 64, 0),
                phase_start_tick: 100,
                distance: 10.0,
            },
        ))
        .id();

    app.update();

    assert!(app.world().get::<JueBiPressureCollapse>(target).is_none());
    assert!(app.world().get::<JueBiLawDisruption>(target).is_some());
}
#[test]
fn juebi_settlement_treats_zero_health_as_killed_even_with_qi() {
    let mut app = qi_test_app();
    let (settings, root) = persistence_settings("juebi-zero-health-settle");
    let char_id = "offline:Azure";
    app.insert_resource(settings);
    app.insert_resource(CombatClock { tick: 600 });
    app.insert_resource(WorldQiBudget::from_total(100.0));
    app.insert_resource(VoidQuotaConfig::default());
    app.add_event::<SkillCapChanged>();
    app.add_event::<TribulationSettled>();
    app.add_event::<AscensionQuotaOccupied>();
    app.add_systems(Update, juebi_settlement_system);

    let entity = app
        .world_mut()
        .spawn((
            Cultivation {
                realm: Realm::Void,
                qi_current: 100.0,
                qi_max: 100.0,
                ..Default::default()
            },
            Wounds {
                health_current: 0.0,
                health_max: 100.0,
                entries: Vec::new(),
            },
            Lifecycle {
                character_id: char_id.to_string(),
                ..Default::default()
            },
            LifeRecord::new(char_id),
            TribulationState {
                kind: TribulationKind::JueBi,
                phase: TribulationPhase::Settle,
                epicenter: [0.0, 64.0, 0.0],
                wave_current: JUEBI_WAVES_TOTAL,
                waves_total: JUEBI_WAVES_TOTAL,
                started_tick: 0,
                phase_started_tick: 600,
                next_wave_tick: 600,
                participants: vec![char_id.to_string()],
                failed: false,
            },
            JueBiRuntimeContext {
                source: JueBiTriggerSource::VoidActionExplodeZone,
                intensity: 1.6,
            },
        ))
        .id();

    app.update();

    let settled = app.world().resource::<Events<TribulationSettled>>();
    let emitted: Vec<_> = settled.get_reader().read(settled).cloned().collect();
    assert_eq!(emitted.len(), 1);
    assert_eq!(emitted[0].kind, TribulationKind::JueBi);
    assert_eq!(emitted[0].result.outcome, DuXuOutcomeV1::Killed);
    let life_record = app
        .world()
        .get::<LifeRecord>(entity)
        .expect("life record should remain attached");
    assert!(life_record
        .biography
        .iter()
        .any(|entry| matches!(entry, BiographyEntry::JueBiKilled { .. })));

    let _ = fs::remove_dir_all(root);
}
#[test]
fn juebi_settlement_clears_independent_active_row() {
    let mut app = qi_test_app();
    let (settings, root) = persistence_settings("juebi-settle-clears-active-row");
    let char_id = "offline:Azure";
    persist_active_tribulation(
        &settings,
        &ActiveTribulationRecord {
            char_id: char_id.to_string(),
            kind: "jue_bi".to_string(),
            source: JueBiTriggerSource::VoidActionExplodeZone
                .wire_name()
                .to_string(),
            origin_dimension: Some("minecraft:overworld".to_string()),
            wave_current: JUEBI_WAVES_TOTAL,
            waves_total: JUEBI_WAVES_TOTAL,
            started_tick: 120,
            epicenter: [0.0, 64.0, 0.0],
            intensity: 1.6,
        },
    )
    .expect("active JueBi should persist before settlement");
    app.insert_resource(settings.clone());
    app.insert_resource(CombatClock { tick: 600 });
    app.insert_resource(WorldQiBudget::from_total(100.0));
    app.insert_resource(VoidQuotaConfig::default());
    app.add_event::<SkillCapChanged>();
    app.add_event::<TribulationSettled>();
    app.add_event::<AscensionQuotaOccupied>();
    app.add_systems(Update, juebi_settlement_system);

    let entity = app
        .world_mut()
        .spawn((
            Cultivation {
                realm: Realm::Void,
                qi_current: 100.0,
                qi_max: 100.0,
                ..Default::default()
            },
            Wounds {
                health_current: 100.0,
                health_max: 100.0,
                entries: Vec::new(),
            },
            Lifecycle {
                character_id: char_id.to_string(),
                ..Default::default()
            },
            LifeRecord::new(char_id),
            TribulationState {
                kind: TribulationKind::JueBi,
                phase: TribulationPhase::Settle,
                epicenter: [0.0, 64.0, 0.0],
                wave_current: JUEBI_WAVES_TOTAL,
                waves_total: JUEBI_WAVES_TOTAL,
                started_tick: 120,
                phase_started_tick: 600,
                next_wave_tick: 600,
                participants: vec![char_id.to_string()],
                failed: false,
            },
            JueBiRuntimeContext {
                source: JueBiTriggerSource::VoidActionExplodeZone,
                intensity: 1.6,
            },
        ))
        .id();

    app.update();

    assert!(app.world().get::<TribulationState>(entity).is_none());
    assert!(
        load_active_tribulation(&settings, char_id)
            .expect("active tribulation query should succeed")
            .is_none(),
        "settled independent JueBi should clear active row"
    );
    let settled = app.world().resource::<Events<TribulationSettled>>();
    let emitted: Vec<_> = settled.get_reader().read(settled).cloned().collect();
    assert_eq!(emitted.len(), 1);
    assert_eq!(emitted[0].kind, TribulationKind::JueBi);
    assert_eq!(emitted[0].result.outcome, DuXuOutcomeV1::HalfStep);

    let _ = fs::remove_dir_all(root);
}
#[test]
fn tribulation_failure_regresses_without_death_lifecycle_side_effects() {
    let mut app = qi_test_app();
    let (settings, root) = persistence_settings("failure-not-death");
    let char_id = "offline:Azure";
    persist_active_tribulation(
        &settings,
        &ActiveTribulationRecord {
            char_id: char_id.to_string(),
            kind: "du_xu".to_string(),
            source: String::new(),
            origin_dimension: Some("minecraft:overworld".to_string()),
            wave_current: 2,
            waves_total: 5,
            started_tick: 120,
            epicenter: [0.0, 64.0, 0.0],
            intensity: 0.0,
        },
    )
    .expect("active tribulation should persist before failure");

    app.insert_resource(settings.clone());
    app.insert_resource(CombatClock { tick: 300 });
    let mut zones = ZoneRegistry::fallback();
    zones
        .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
        .expect("fallback zone should exist")
        .spirit_qi = 0.25;
    app.insert_resource(zones);
    app.add_event::<TribulationFailed>();
    app.add_event::<TribulationFled>();
    app.add_event::<TribulationSettled>();
    app.add_event::<DeathEvent>();
    app.add_event::<CultivationDeathTrigger>();
    app.add_event::<DeathInsightRequested>();
    app.add_event::<PlayerTerminated>();
    app.add_event::<QiTransfer>();
    app.add_event::<VfxEventRequest>();
    app.add_systems(
        Update,
        (
            tribulation_failure_system,
            death_arbiter_tick.after(tribulation_failure_system),
        ),
    );

    let entity = app
        .world_mut()
        .spawn((
            Cultivation {
                realm: Realm::Spirit,
                qi_current: 210.0,
                qi_max: 210.0,
                last_qi_zero_at: Some(77),
                pending_material_bonus: 0.3,
                ..Default::default()
            },
            all_meridians_open(),
            Wounds {
                health_current: 0.0,
                health_max: 100.0,
                entries: Vec::new(),
            },
            Stamina::default(),
            CombatState::default(),
            Lifecycle {
                character_id: char_id.to_string(),
                death_count: 2,
                last_death_tick: Some(55),
                state: LifecycleState::Alive,
                ..Default::default()
            },
            DeathRegistry {
                char_id: char_id.to_string(),
                death_count: 2,
                last_death_tick: Some(55),
                prev_death_tick: Some(12),
                last_death_zone: Some(ZoneDeathKind::Ordinary),
            },
            LifespanComponent {
                born_at_tick: 0,
                years_lived: 90.0,
                cap_by_realm: LifespanCapTable::SPIRIT,
                offline_pause_tick: None,
            },
            LifeRecord::new(char_id),
            Position::new([8.0, 66.0, 8.0]),
            CurrentDimension(DimensionKind::Overworld),
            TribulationState::restored(2, 5, 120),
        ))
        .id();
    let zone_before = app
        .world()
        .resource::<ZoneRegistry>()
        .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
        .expect("fallback zone should exist")
        .spirit_qi;

    app.world_mut()
        .resource_mut::<Events<TribulationFailed>>()
        .send(TribulationFailed { entity, wave: 3 });
    app.update();

    let entity_ref = app.world().entity(entity);
    let cultivation = entity_ref
        .get::<Cultivation>()
        .expect("cultivation should remain attached");
    let meridians = entity_ref
        .get::<MeridianSystem>()
        .expect("meridians should remain attached");
    let wounds = entity_ref
        .get::<Wounds>()
        .expect("wounds should remain attached");
    let lifecycle = entity_ref
        .get::<Lifecycle>()
        .expect("lifecycle should remain attached");
    let registry = entity_ref
        .get::<DeathRegistry>()
        .expect("death registry should remain attached");
    let lifespan = entity_ref
        .get::<LifespanComponent>()
        .expect("lifespan should remain attached");

    assert_eq!(cultivation.realm, Realm::Spirit);
    assert_eq!(cultivation.qi_current, 0.0);
    assert_eq!(cultivation.last_qi_zero_at, None);
    assert_eq!(cultivation.pending_material_bonus, 0.0);
    assert_eq!(meridians.opened_count(), Realm::Spirit.required_meridians());
    assert_eq!(cultivation.qi_max, 10.0 + meridians.sum_capacity());
    assert!(wounds.health_current > 0.0);
    assert_eq!(lifecycle.state, LifecycleState::Alive);
    assert_eq!(lifecycle.death_count, 2);
    assert_eq!(lifecycle.last_death_tick, Some(55));
    assert_eq!(registry.death_count, 2);
    assert_eq!(registry.last_death_tick, Some(55));
    assert_eq!(lifespan.years_lived, 90.0);
    assert!(entity_ref.get::<TribulationState>().is_none());
    let zone_after = app
        .world()
        .resource::<ZoneRegistry>()
        .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
        .expect("fallback zone should exist")
        .spirit_qi;
    let transfers: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<QiTransfer>>()
        .drain()
        .collect();

    assert_eq!(
        app.world()
            .resource::<Events<CultivationDeathTrigger>>()
            .len(),
        0
    );
    assert_eq!(
        app.world()
            .resource::<Events<DeathInsightRequested>>()
            .len(),
        0
    );
    assert_eq!(app.world().resource::<Events<PlayerTerminated>>().len(), 0);
    assert!(
        zone_after > zone_before,
        "tribulation failure should release cleared qi back to the current zone"
    );
    assert_eq!(transfers.len(), 2);
    assert_eq!(transfers[0].to, qi_flow_overflow_account());
    assert_eq!(transfers[0].reason, QiTransferReason::ReleaseToZone);
    assert_eq!(transfers[1].reason, QiTransferReason::ReleaseToZone);
    assert_eq!(transfers[1].to, QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME));
    assert!(
        load_active_tribulation(&settings, char_id)
            .expect("active tribulation query should succeed")
            .is_none(),
        "failed tribulation should clear active row"
    );

    let _ = fs::remove_dir_all(root);
}
#[test]
fn intercepted_tribulation_transfers_all_inventory_to_killer() {
    let mut app = qi_test_app();
    let (settings, root) = persistence_settings("intercept-loot-transfer");
    app.insert_resource(settings.clone());
    app.insert_resource(ItemRegistry::default());
    app.add_event::<DeathEvent>();
    app.add_event::<TribulationSettled>();
    app.add_systems(Update, tribulation_intercept_death_system);

    let victim = app
        .world_mut()
        .spawn((
            Lifecycle {
                character_id: "offline:Victim".to_string(),
                ..Default::default()
            },
            TribulationState {
                kind: TribulationKind::DuXu,
                phase: TribulationPhase::Wave(2),
                epicenter: [0.0, 66.0, 0.0],
                wave_current: 2,
                waves_total: 3,
                started_tick: 0,
                phase_started_tick: 0,
                next_wave_tick: 0,
                participants: vec!["offline:Victim".to_string(), "offline:Killer".to_string()],
                failed: false,
            },
            test_inventory(vec![test_item(101), test_item(102)], 7),
        ))
        .id();
    let killer = app
        .world_mut()
        .spawn((
            test_inventory(vec![test_item(201)], 3),
            LifeRecord::new("offline:Killer"),
        ))
        .id();

    app.world_mut().send_event(DeathEvent {
        target: victim,
        cause: "pvp:offline:Killer".to_string(),
        attacker: Some(killer),
        attacker_player_id: Some("offline:Killer".to_string()),
        at_tick: 120,
    });

    app.update();

    let victim_inventory = app
        .world()
        .get::<PlayerInventory>(victim)
        .expect("victim inventory should remain attached");
    assert_eq!(victim_inventory.bone_coins, 0);
    assert!(victim_inventory
        .containers
        .iter()
        .all(|container| container.items.is_empty()));
    assert!(victim_inventory.equipped.is_empty());
    assert!(victim_inventory.hotbar.iter().all(Option::is_none));

    let killer_inventory = app
        .world()
        .get::<PlayerInventory>(killer)
        .expect("killer inventory should remain attached");
    assert_eq!(killer_inventory.bone_coins, 10);
    let killer_item_ids = killer_inventory
        .containers
        .iter()
        .flat_map(|container| container.items.iter())
        .map(|placed| placed.instance.instance_id)
        .collect::<Vec<_>>();
    assert!(killer_item_ids.contains(&101));
    assert!(killer_item_ids.contains(&102));
    assert!(killer_item_ids.contains(&201));

    let killer_life_record = app
        .world()
        .get::<LifeRecord>(killer)
        .expect("killer life record should remain attached");
    assert!(matches!(
        killer_life_record.biography.last(),
        Some(BiographyEntry::TribulationIntercepted { victim_id, tag, tick })
            if victim_id == "offline:Victim" && tag == "戮道者 · 截劫" && *tick == 120
    ));

    assert!(app.world().get::<TribulationState>(victim).is_none());
    let settled = app.world().resource::<Events<TribulationSettled>>();
    let emitted: Vec<_> = settled.get_reader().read(settled).cloned().collect();
    assert_eq!(emitted.len(), 1);
    assert_eq!(emitted[0].result.outcome, DuXuOutcomeV1::Killed);
    assert_eq!(emitted[0].result.killer.as_deref(), Some("offline:Killer"));

    let _ = fs::remove_dir_all(root);
}
#[test]
fn npc_raw_death_does_not_settle_tribulation_before_terminal_commit() {
    let mut app = qi_test_app();
    let (settings, root) = persistence_settings("npc-intercept-waits-for-terminal-commit");
    app.insert_resource(settings.clone());
    app.insert_resource(ItemRegistry::default());
    app.add_event::<DeathEvent>();
    app.add_event::<TribulationSettled>();
    app.add_systems(Update, tribulation_intercept_death_system);

    let victim = app
        .world_mut()
        .spawn((
            bong_server::npc::spawn::NpcMarker,
            Lifecycle {
                character_id: "npc:tribulation-victim".to_string(),
                ..Default::default()
            },
            TribulationState {
                kind: TribulationKind::DuXu,
                phase: TribulationPhase::Wave(2),
                epicenter: [0.0, 66.0, 0.0],
                wave_current: 2,
                waves_total: 3,
                started_tick: 0,
                phase_started_tick: 0,
                next_wave_tick: 0,
                participants: vec![
                    "npc:tribulation-victim".to_string(),
                    "offline:Killer".to_string(),
                ],
                failed: false,
            },
            test_inventory(vec![test_item(101)], 7),
        ))
        .id();
    let killer = app
        .world_mut()
        .spawn((
            test_inventory(vec![test_item(201)], 3),
            LifeRecord::new("offline:Killer"),
        ))
        .id();

    app.world_mut().send_event(DeathEvent {
        target: victim,
        cause: "pvp:offline:Killer".to_string(),
        attacker: Some(killer),
        attacker_player_id: Some("offline:Killer".to_string()),
        at_tick: 120,
    });
    app.update();

    assert!(
        app.world().get::<TribulationState>(victim).is_some(),
        "NPC raw lethal edge must retain tribulation state until terminal durability succeeds"
    );
    assert_eq!(
        app.world()
            .get::<PlayerInventory>(victim)
            .expect("victim inventory should remain attached")
            .bone_coins,
        7,
        "NPC raw lethal edge must not transfer victim inventory"
    );
    assert_eq!(
        app.world()
            .get::<PlayerInventory>(killer)
            .expect("killer inventory should remain attached")
            .bone_coins,
        3,
        "NPC raw lethal edge must not reward the killer"
    );
    assert!(
        app.world()
            .get::<LifeRecord>(killer)
            .expect("killer life record should remain attached")
            .biography
            .is_empty(),
        "NPC raw lethal edge must not append interception biography"
    );
    assert_eq!(
        app.world().resource::<Events<TribulationSettled>>().len(),
        0,
        "NPC raw lethal edge must not publish settlement before terminal commit"
    );

    let _ = fs::remove_dir_all(root);
}
#[test]
fn unregistered_player_kill_does_not_claim_interception_settlement() {
    let mut app = qi_test_app();
    let (settings, root) = persistence_settings("intercept-killer-must-be-participant");
    app.insert_resource(settings);
    app.insert_resource(ItemRegistry::default());
    app.add_event::<DeathEvent>();
    app.add_event::<TribulationSettled>();
    app.add_systems(Update, tribulation_intercept_death_system);

    let victim = app
        .world_mut()
        .spawn((
            Lifecycle {
                character_id: "offline:Victim".to_string(),
                ..Default::default()
            },
            TribulationState {
                kind: TribulationKind::DuXu,
                phase: TribulationPhase::Wave(2),
                epicenter: [0.0, 66.0, 0.0],
                wave_current: 2,
                waves_total: 3,
                started_tick: 0,
                phase_started_tick: 0,
                next_wave_tick: 0,
                participants: vec!["offline:Victim".to_string()],
                failed: false,
            },
            test_inventory(vec![test_item(101)], 7),
        ))
        .id();
    let killer = app
        .world_mut()
        .spawn((
            test_inventory(vec![test_item(201)], 3),
            LifeRecord::new("offline:Killer"),
        ))
        .id();

    app.world_mut().send_event(DeathEvent {
        target: victim,
        cause: "pvp:offline:Killer".to_string(),
        attacker: Some(killer),
        attacker_player_id: Some("offline:Killer".to_string()),
        at_tick: 120,
    });

    app.update();

    assert!(app.world().get::<TribulationState>(victim).is_some());
    assert_eq!(
        app.world().resource::<Events<TribulationSettled>>().len(),
        0
    );
    let victim_inventory = app.world().get::<PlayerInventory>(victim).unwrap();
    assert_eq!(victim_inventory.bone_coins, 7);
    let killer_inventory = app.world().get::<PlayerInventory>(killer).unwrap();
    assert_eq!(killer_inventory.bone_coins, 3);

    let _ = fs::remove_dir_all(root);
}
#[test]
fn attacking_locked_tribulator_records_interceptor_participant() {
    let mut app = qi_test_app();
    app.add_event::<CombatEvent>();
    app.add_systems(Update, record_tribulation_interceptor_system);

    let victim = app
        .world_mut()
        .spawn((
            Position::new([0.0, 66.0, 0.0]),
            Lifecycle {
                character_id: "offline:Victim".to_string(),
                ..Default::default()
            },
            TribulationState {
                kind: TribulationKind::DuXu,
                phase: TribulationPhase::Lock,
                epicenter: [0.0, 66.0, 0.0],
                wave_current: 0,
                waves_total: 3,
                started_tick: 0,
                phase_started_tick: 0,
                next_wave_tick: 0,
                participants: vec!["offline:Victim".to_string()],
                failed: false,
            },
        ))
        .id();
    let interceptor = app
        .world_mut()
        .spawn((
            Position::new([12.0, 66.0, 0.0]),
            Lifecycle {
                character_id: "offline:Killer".to_string(),
                ..Default::default()
            },
        ))
        .id();

    for _ in 0..2 {
        app.world_mut().send_event(CombatEvent {
            attacker: interceptor,
            target: victim,
            resolved_at_tick: 120,
            body_part: BodyPart::Chest,
            wound_kind: WoundKind::Cut,
            source: bong_server::combat::events::AttackSource::Melee,
            debug_command: false,
            physical_damage: 0.0,
            damage: 12.0,
            contam_delta: 0.0,
            description: "test interception hit".to_string(),
            defense_kind: None,
            defense_effectiveness: None,
            defense_contam_reduced: None,
            defense_wound_severity: None,
        });
    }
    app.update();

    let state = app
        .world()
        .get::<TribulationState>(victim)
        .expect("tribulation should remain active");
    assert_eq!(
        state.participants,
        vec!["offline:Victim".to_string(), "offline:Killer".to_string()]
    );
}
#[test]
fn attacking_during_heart_demon_records_interceptor_participant() {
    let mut app = qi_test_app();
    app.add_event::<CombatEvent>();
    app.add_systems(Update, record_tribulation_interceptor_system);

    let victim = app
        .world_mut()
        .spawn((
            Position::new([0.0, 66.0, 0.0]),
            Lifecycle {
                character_id: "offline:Victim".to_string(),
                ..Default::default()
            },
            TribulationState {
                kind: TribulationKind::DuXu,
                phase: TribulationPhase::HeartDemon,
                epicenter: [0.0, 66.0, 0.0],
                wave_current: 4,
                waves_total: 5,
                started_tick: 0,
                phase_started_tick: 2100,
                next_wave_tick: 2400,
                participants: vec!["offline:Victim".to_string()],
                failed: false,
            },
        ))
        .id();
    let interceptor = app
        .world_mut()
        .spawn((
            Position::new([12.0, 66.0, 0.0]),
            Lifecycle {
                character_id: "offline:Killer".to_string(),
                ..Default::default()
            },
        ))
        .id();

    app.world_mut().send_event(CombatEvent {
        attacker: interceptor,
        target: victim,
        resolved_at_tick: 2130,
        body_part: BodyPart::Chest,
        wound_kind: WoundKind::Cut,
        source: bong_server::combat::events::AttackSource::Melee,
        debug_command: false,
        physical_damage: 0.0,
        damage: 12.0,
        contam_delta: 0.0,
        description: "test heart demon interception hit".to_string(),
        defense_kind: None,
        defense_effectiveness: None,
        defense_contam_reduced: None,
        defense_wound_severity: None,
    });
    app.update();

    let state = app
        .world()
        .get::<TribulationState>(victim)
        .expect("tribulation should remain active");
    assert_eq!(
        state.participants,
        vec!["offline:Victim".to_string(), "offline:Killer".to_string()]
    );
}
#[test]
fn attacking_tribulator_from_other_dimension_does_not_record_interceptor() {
    let mut app = qi_test_app();
    app.add_event::<CombatEvent>();
    app.add_systems(Update, record_tribulation_interceptor_system);

    let victim = app
        .world_mut()
        .spawn((
            Position::new([0.0, 66.0, 0.0]),
            CurrentDimension(DimensionKind::Overworld),
            Lifecycle {
                character_id: "offline:Victim".to_string(),
                ..Default::default()
            },
            TribulationState {
                kind: TribulationKind::DuXu,
                phase: TribulationPhase::Lock,
                epicenter: [0.0, 66.0, 0.0],
                wave_current: 0,
                waves_total: 3,
                started_tick: 0,
                phase_started_tick: 120,
                next_wave_tick: 300,
                participants: vec!["offline:Victim".to_string()],
                failed: false,
            },
        ))
        .id();
    let interceptor = app
        .world_mut()
        .spawn((
            Position::new([12.0, 66.0, 0.0]),
            CurrentDimension(DimensionKind::Tsy),
            Lifecycle {
                character_id: "offline:Killer".to_string(),
                ..Default::default()
            },
        ))
        .id();

    app.world_mut().send_event(CombatEvent {
        attacker: interceptor,
        target: victim,
        resolved_at_tick: 120,
        body_part: BodyPart::Chest,
        wound_kind: WoundKind::Cut,
        source: bong_server::combat::events::AttackSource::Melee,
        debug_command: false,
        physical_damage: 0.0,
        damage: 12.0,
        contam_delta: 0.0,
        description: "test cross-dimension interception hit".to_string(),
        defense_kind: None,
        defense_effectiveness: None,
        defense_contam_reduced: None,
        defense_wound_severity: None,
    });
    app.update();

    let state = app
        .world()
        .get::<TribulationState>(victim)
        .expect("tribulation should remain active");
    assert_eq!(state.participants, vec!["offline:Victim".to_string()]);
}
#[test]
fn attacking_restored_tribulator_preserves_primary_participant() {
    let mut app = qi_test_app();
    app.add_event::<CombatEvent>();
    app.add_systems(Update, record_tribulation_interceptor_system);

    let victim = app
        .world_mut()
        .spawn((
            Position::new([0.0, 66.0, 0.0]),
            Lifecycle {
                character_id: "offline:Victim".to_string(),
                ..Default::default()
            },
            TribulationState::restored(1, 3, 0),
        ))
        .id();
    let interceptor = app
        .world_mut()
        .spawn((
            Position::new([12.0, 66.0, 0.0]),
            Lifecycle {
                character_id: "offline:Killer".to_string(),
                ..Default::default()
            },
        ))
        .id();

    app.world_mut().send_event(CombatEvent {
        attacker: interceptor,
        target: victim,
        resolved_at_tick: 120,
        body_part: BodyPart::Chest,
        wound_kind: WoundKind::Cut,
        source: bong_server::combat::events::AttackSource::Melee,
        debug_command: false,
        physical_damage: 0.0,
        damage: 12.0,
        contam_delta: 0.0,
        description: "test restored interception hit".to_string(),
        defense_kind: None,
        defense_effectiveness: None,
        defense_contam_reduced: None,
        defense_wound_severity: None,
    });
    app.update();

    let state = app
        .world()
        .get::<TribulationState>(victim)
        .expect("tribulation should remain active");
    assert_eq!(
        state.participants,
        vec!["offline:Victim".to_string(), "offline:Killer".to_string()]
    );
}
#[test]
fn registered_interceptor_dies_to_aoe_without_failing_tribulation() {
    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 300 });
    app.add_event::<TribulationFailed>();
    app.add_event::<DeathEvent>();
    app.add_systems(Update, tribulation_aoe_system);

    app.world_mut().spawn((
        Position::new([0.0, 66.0, 0.0]),
        Cultivation {
            realm: Realm::Spirit,
            qi_current: 120.0,
            qi_max: 210.0,
            ..Default::default()
        },
        Wounds {
            health_current: 100.0,
            health_max: 100.0,
            entries: Vec::new(),
        },
        Lifecycle {
            character_id: "offline:Victim".to_string(),
            ..Default::default()
        },
        LifeRecord::new("offline:Victim"),
        TribulationState {
            kind: TribulationKind::DuXu,
            phase: TribulationPhase::Wave(1),
            epicenter: [0.0, 66.0, 0.0],
            wave_current: 1,
            waves_total: 3,
            started_tick: 0,
            phase_started_tick: 300,
            next_wave_tick: 300,
            participants: vec!["offline:Victim".to_string(), "offline:Killer".to_string()],
            failed: false,
        },
    ));
    let interceptor = app
        .world_mut()
        .spawn((
            Position::new([8.0, 66.0, 0.0]),
            Cultivation {
                realm: Realm::Spirit,
                qi_current: 50.0,
                qi_max: 80.0,
                ..Default::default()
            },
            Wounds {
                health_current: 1.0,
                health_max: 100.0,
                entries: Vec::new(),
            },
            Lifecycle {
                character_id: "offline:Killer".to_string(),
                ..Default::default()
            },
            LifeRecord::new("offline:Killer"),
        ))
        .id();

    app.update();

    assert_eq!(app.world().resource::<Events<TribulationFailed>>().len(), 0);
    let deaths = app.world().resource::<Events<DeathEvent>>();
    let emitted: Vec<_> = deaths.get_reader().read(deaths).cloned().collect();
    assert_eq!(emitted.len(), 1);
    assert_eq!(emitted[0].target, interceptor);
    assert_eq!(emitted[0].cause, "观劫而亡");
    assert_eq!(emitted[0].attacker_player_id, None);
}
#[test]
fn spectator_death_by_tribulation_aoe_is_written_to_life_record() {
    let mut app = qi_test_app();
    let (settings, root) = persistence_settings("spectator-death-biography");
    app.insert_resource(settings);
    app.insert_resource(CombatClock { tick: 300 });
    app.add_event::<TribulationFailed>();
    app.add_event::<DeathEvent>();
    app.add_event::<CultivationDeathTrigger>();
    app.add_event::<DeathInsightRequested>();
    app.add_event::<PlayerTerminated>();
    app.add_event::<VfxEventRequest>();
    app.add_systems(
        Update,
        (
            tribulation_aoe_system,
            death_arbiter_tick.after(tribulation_aoe_system),
        ),
    );

    app.world_mut().spawn((
        Position::new([0.0, 66.0, 0.0]),
        Cultivation {
            realm: Realm::Spirit,
            qi_current: 120.0,
            qi_max: 210.0,
            ..Default::default()
        },
        Wounds {
            health_current: 100.0,
            health_max: 100.0,
            entries: Vec::new(),
        },
        Lifecycle {
            character_id: "offline:Victim".to_string(),
            state: LifecycleState::Alive,
            ..Default::default()
        },
        TribulationState {
            kind: TribulationKind::DuXu,
            phase: TribulationPhase::Wave(1),
            epicenter: [0.0, 66.0, 0.0],
            wave_current: 1,
            waves_total: 3,
            started_tick: 0,
            phase_started_tick: 300,
            next_wave_tick: 300,
            participants: vec!["offline:Victim".to_string()],
            failed: false,
        },
    ));
    let spectator = app
        .world_mut()
        .spawn((
            Position::new([20.0, 66.0, 0.0]),
            Cultivation {
                realm: Realm::Awaken,
                qi_current: 10.0,
                qi_max: 40.0,
                ..Default::default()
            },
            Wounds {
                health_current: 1.0,
                health_max: 100.0,
                entries: Vec::new(),
            },
            Stamina::default(),
            CombatState::default(),
            Lifecycle {
                character_id: "offline:Spectator".to_string(),
                state: LifecycleState::Alive,
                fortune_remaining: 1,
                ..Default::default()
            },
            DeathRegistry::new("offline:Spectator".to_string()),
            LifespanComponent::new(LifespanCapTable::AWAKEN),
            LifeRecord::new("offline:Spectator"),
        ))
        .id();

    app.update();

    let lifecycle = app
        .world()
        .get::<Lifecycle>(spectator)
        .expect("spectator lifecycle should remain attached");
    assert_eq!(lifecycle.state, LifecycleState::NearDeath);
    let life = app
        .world()
        .get::<LifeRecord>(spectator)
        .expect("spectator life record should remain attached");
    assert!(matches!(
        life.biography.last(),
        Some(BiographyEntry::NearDeath { cause, tick }) if cause == "观劫而亡" && *tick == 300
    ));
    assert_eq!(app.world().resource::<Events<TribulationFailed>>().len(), 0);

    let _ = fs::remove_dir_all(root);
}
#[test]
fn disconnecting_during_tribulation_flees_and_regresses_without_death() {
    let mut app = qi_test_app();
    let (settings, root) = persistence_settings("disconnect-fled");
    let char_id = "offline:Azure";
    persist_active_tribulation(
        &settings,
        &ActiveTribulationRecord {
            char_id: char_id.to_string(),
            kind: "du_xu".to_string(),
            source: String::new(),
            origin_dimension: Some("minecraft:overworld".to_string()),
            wave_current: 1,
            waves_total: 3,
            started_tick: 80,
            epicenter: [0.0, 64.0, 0.0],
            intensity: 0.0,
        },
    )
    .expect("active tribulation should persist before disconnect");

    app.insert_resource(settings.clone());
    app.insert_resource(CombatClock { tick: 320 });
    app.add_event::<TribulationSettled>();
    app.add_event::<TribulationFled>();
    app.add_systems(Update, abort_du_xu_on_client_removed);

    let (mut client_bundle, _helper) = create_mock_client("Azure");
    client_bundle.player.position = Position::new([8.0, 66.0, 8.0]);
    let entity = app
        .world_mut()
        .spawn((
            client_bundle,
            Cultivation {
                realm: Realm::Spirit,
                qi_current: 120.0,
                qi_max: 210.0,
                ..Default::default()
            },
            all_meridians_open(),
            Wounds {
                health_current: 0.0,
                health_max: 100.0,
                entries: Vec::new(),
            },
            Lifecycle {
                character_id: char_id.to_string(),
                state: LifecycleState::Alive,
                ..Default::default()
            },
            LifeRecord::new(char_id),
            TribulationState {
                kind: TribulationKind::DuXu,
                phase: TribulationPhase::Wave(2),
                epicenter: [0.0, 66.0, 0.0],
                wave_current: 1,
                waves_total: 3,
                started_tick: 80,
                phase_started_tick: 280,
                next_wave_tick: 320,
                participants: vec![char_id.to_string()],
                failed: false,
            },
        ))
        .id();

    app.world_mut().entity_mut(entity).remove::<Client>();
    app.update();

    assert!(app.world().get::<TribulationState>(entity).is_none());
    let cultivation = app
        .world()
        .get::<Cultivation>(entity)
        .expect("cultivation should remain attached");
    assert_eq!(cultivation.realm, Realm::Spirit);
    assert_eq!(cultivation.qi_current, 0.0);
    let life = app
        .world()
        .get::<LifeRecord>(entity)
        .expect("life record should remain attached");
    assert!(matches!(
        life.biography.last(),
        Some(BiographyEntry::TribulationFled { wave: 2, tick: 320 })
    ));

    let settled: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<TribulationSettled>>()
        .drain()
        .collect();
    assert_eq!(settled.len(), 1);
    assert_eq!(settled[0].result.outcome, DuXuOutcomeV1::Fled);
    assert_eq!(settled[0].result.waves_survived, 1);
    let fled: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<TribulationFled>>()
        .drain()
        .collect();
    assert_eq!(fled.len(), 1);
    assert_eq!(fled[0].entity, entity);
    assert_eq!(fled[0].tick, 320);
    assert!(
        load_active_tribulation(&settings, char_id)
            .expect("active tribulation query should succeed")
            .is_none(),
        "fled tribulation should clear active row"
    );

    let _ = fs::remove_dir_all(root);
}
#[test]
fn juebi_disconnect_is_settled_as_fled() {
    let mut app = qi_test_app();
    let (settings, root) = persistence_settings("juebi-disconnect-fled");
    let char_id = "offline:JueBiPlayer";
    persist_active_tribulation(
        &settings,
        &ActiveTribulationRecord {
            char_id: char_id.to_string(),
            kind: "jue_bi".to_string(),
            source: "void_quota_exceeded".to_string(),
            origin_dimension: Some("minecraft:overworld".to_string()),
            wave_current: 2,
            waves_total: 5,
            started_tick: 100,
            epicenter: [0.0, 64.0, 0.0],
            intensity: 1.0,
        },
    )
    .expect("active jue_bi tribulation should persist before disconnect");

    app.insert_resource(settings.clone());
    app.insert_resource(CombatClock { tick: 500 });
    app.add_event::<TribulationSettled>();
    app.add_event::<TribulationFled>();
    app.add_systems(Update, abort_du_xu_on_client_removed);

    let (mut client_bundle, _helper) = create_mock_client("JueBiPlayer");
    client_bundle.player.position = Position::new([8.0, 66.0, 8.0]);
    let entity = app
        .world_mut()
        .spawn((
            client_bundle,
            Cultivation {
                realm: Realm::Spirit,
                qi_current: 100.0,
                qi_max: 210.0,
                ..Default::default()
            },
            all_meridians_open(),
            Wounds {
                health_current: 80.0,
                health_max: 100.0,
                entries: Vec::new(),
            },
            Lifecycle {
                character_id: char_id.to_string(),
                state: LifecycleState::Alive,
                ..Default::default()
            },
            LifeRecord::new(char_id),
            TribulationState {
                kind: TribulationKind::JueBi,
                phase: TribulationPhase::Wave(2),
                epicenter: [0.0, 66.0, 0.0],
                wave_current: 2,
                waves_total: 5,
                started_tick: 100,
                phase_started_tick: 400,
                next_wave_tick: 600,
                participants: vec![char_id.to_string()],
                failed: false,
            },
        ))
        .id();

    // 触发断线：移除 Client 组件
    app.world_mut().entity_mut(entity).remove::<Client>();
    app.update();

    // ① TribulationState 组件已被移除（entity 不再处于劫中）
    assert!(
        app.world().get::<TribulationState>(entity).is_none(),
        "期望 TribulationState 被移除（fled 结算完成），实际仍附着在 entity 上"
    );

    // ② TribulationFled 事件已发送（fled 后果）
    let fled: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<TribulationFled>>()
        .drain()
        .collect();
    assert_eq!(
        fled.len(),
        1,
        "期望 1 个 TribulationFled 事件（JueBi 断线 = fled），实际 {}",
        fled.len()
    );
    assert_eq!(
        fled[0].entity, entity,
        "期望 fled 事件绑定断线玩家 entity，实际 {:?}",
        fled[0].entity
    );
    assert_eq!(
        fled[0].tick, 500,
        "期望 fled_tick = CombatClock.tick = 500，实际 {}",
        fled[0].tick
    );

    // ③ TribulationSettled.kind == JueBi（不误报 DuXu）
    let settled: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<TribulationSettled>>()
        .drain()
        .collect();
    assert_eq!(
        settled.len(),
        1,
        "期望 1 个 TribulationSettled 事件，实际 {}",
        settled.len()
    );
    assert_eq!(
        settled[0].kind,
        TribulationKind::JueBi,
        "期望 TribulationSettled.kind == JueBi（不误报 DuXu），实际 {:?}",
        settled[0].kind
    );
    assert_eq!(
        settled[0].result.outcome,
        DuXuOutcomeV1::Fled,
        "期望结算结果 = Fled，实际 {:?}",
        settled[0].result.outcome
    );
    assert_eq!(
        settled[0].result.waves_survived, 2,
        "期望 waves_survived = wave_current(2)，实际 {}",
        settled[0].result.waves_survived
    );

    // ④ LifeRecord 记录了 TribulationFled 传记条目（fled 后果落到玩家持久化态）
    let life = app
        .world()
        .get::<LifeRecord>(entity)
        .expect("LifeRecord 应仍附着在 entity 上");
    assert!(
        matches!(
            life.biography.last(),
            Some(BiographyEntry::TribulationFled { wave: 3, tick: 500 })
        ),
        "期望最后一条传记条目为 TribulationFled {{ wave: 3, tick: 500 }}（wave_current.saturating_add(1)），实际 {:?}",
        life.biography.last()
    );

    // ⑤ active_tribulations 表行已被删除（无 orphan）
    assert!(
        load_active_tribulation(&settings, char_id)
            .expect("active tribulation 查询应成功")
            .is_none(),
        "期望 JueBi 断线后 tribulations_active 行被删除（无孤儿），实际行仍存在"
    );

    let _ = fs::remove_dir_all(root);
}
#[test]
fn non_tribulation_disconnect_emits_no_fled_events() {
    let mut app = qi_test_app();
    let (settings, root) = persistence_settings("no-trib-disconnect");
    let char_id = "offline:NormalPlayer";

    app.insert_resource(settings.clone());
    app.insert_resource(CombatClock { tick: 200 });
    app.add_event::<TribulationSettled>();
    app.add_event::<TribulationFled>();
    app.add_systems(Update, abort_du_xu_on_client_removed);

    let (client_bundle, _helper) = create_mock_client("NormalPlayer");
    let entity = app
        .world_mut()
        .spawn((
            client_bundle,
            Cultivation {
                realm: Realm::Spirit,
                qi_current: 80.0,
                qi_max: 100.0,
                ..Default::default()
            },
            Lifecycle {
                character_id: char_id.to_string(),
                state: LifecycleState::Alive,
                ..Default::default()
            },
            // 刻意不挂 TribulationState
        ))
        .id();

    app.world_mut().entity_mut(entity).remove::<Client>();
    app.update();

    let fled: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<TribulationFled>>()
        .drain()
        .collect();
    let settled: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<TribulationSettled>>()
        .drain()
        .collect();
    assert_eq!(
        fled.len(),
        0,
        "期望无 TribulationFled 事件（非渡劫玩家不应被误结算），实际 {} 个",
        fled.len()
    );
    assert_eq!(
        settled.len(),
        0,
        "期望无 TribulationSettled 事件（非渡劫玩家不应被误结算），实际 {} 个",
        settled.len()
    );

    let _ = fs::remove_dir_all(root);
}
#[test]
fn settle_fled_emits_correct_kind_for_juebi() {
    let mut app = qi_test_app();
    let (settings, root) = persistence_settings("juebi-settled-kind");
    let char_id = "offline:KindCheck";
    persist_active_tribulation(
        &settings,
        &ActiveTribulationRecord {
            char_id: char_id.to_string(),
            kind: "jue_bi".to_string(),
            source: String::new(),
            origin_dimension: None,
            wave_current: 0,
            waves_total: 3,
            started_tick: 10,
            epicenter: [0.0, 64.0, 0.0],
            intensity: 0.0,
        },
    )
    .expect("active jue_bi row 应能写入");

    app.insert_resource(settings.clone());
    app.insert_resource(CombatClock { tick: 50 });
    app.add_event::<TribulationSettled>();
    app.add_event::<TribulationFled>();
    app.add_systems(Update, abort_du_xu_on_client_removed);

    let (mut client_bundle, _helper) = create_mock_client("KindCheck");
    client_bundle.player.position = Position::new([0.0, 64.0, 0.0]);
    let entity = app
        .world_mut()
        .spawn((
            client_bundle,
            Cultivation {
                realm: Realm::Spirit,
                qi_current: 50.0,
                qi_max: 100.0,
                ..Default::default()
            },
            all_meridians_open(),
            Wounds::default(),
            Lifecycle {
                character_id: char_id.to_string(),
                state: LifecycleState::Alive,
                ..Default::default()
            },
            LifeRecord::new(char_id),
            TribulationState {
                kind: TribulationKind::JueBi,
                phase: TribulationPhase::Wave(0),
                epicenter: [0.0, 64.0, 0.0],
                wave_current: 0,
                waves_total: 3,
                started_tick: 10,
                phase_started_tick: 10,
                next_wave_tick: 100,
                participants: vec![char_id.to_string()],
                failed: false,
            },
        ))
        .id();

    app.world_mut().entity_mut(entity).remove::<Client>();
    app.update();

    let settled: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<TribulationSettled>>()
        .drain()
        .collect();
    assert_eq!(settled.len(), 1);
    assert_eq!(
        settled[0].kind,
        TribulationKind::JueBi,
        "期望 TribulationSettled.kind == JueBi（settle_fled_tribulation 应用 state.kind 而非硬编码 DuXu），实际 {:?}",
        settled[0].kind
    );
    assert_ne!(
        settled[0].kind,
        TribulationKind::DuXu,
        "不期望 kind == DuXu（这是回归：hardcoded kind bug 复现）"
    );

    let _ = fs::remove_dir_all(root);
}
#[test]
fn juebi_disconnect_at_wave_zero_is_fled() {
    let mut app = qi_test_app();
    let (settings, root) = persistence_settings("juebi-wave0-fled");
    let char_id = "offline:EarlyEscape";
    persist_active_tribulation(
        &settings,
        &ActiveTribulationRecord {
            char_id: char_id.to_string(),
            kind: "jue_bi".to_string(),
            source: String::new(),
            origin_dimension: None,
            wave_current: 0,
            waves_total: 5,
            started_tick: 1000,
            epicenter: [0.0, 64.0, 0.0],
            intensity: 0.0,
        },
    )
    .expect("active jue_bi 应能写入");

    app.insert_resource(settings.clone());
    app.insert_resource(CombatClock { tick: 1001 });
    app.add_event::<TribulationSettled>();
    app.add_event::<TribulationFled>();
    app.add_systems(Update, abort_du_xu_on_client_removed);

    let (mut client_bundle, _helper) = create_mock_client("EarlyEscape");
    client_bundle.player.position = Position::new([0.0, 64.0, 0.0]);
    let entity = app
        .world_mut()
        .spawn((
            client_bundle,
            Cultivation {
                realm: Realm::Spirit,
                qi_current: 200.0,
                qi_max: 210.0,
                ..Default::default()
            },
            all_meridians_open(),
            Wounds::default(),
            Lifecycle {
                character_id: char_id.to_string(),
                state: LifecycleState::Alive,
                ..Default::default()
            },
            LifeRecord::new(char_id),
            TribulationState {
                kind: TribulationKind::JueBi,
                phase: TribulationPhase::Wave(0),
                epicenter: [0.0, 64.0, 0.0],
                wave_current: 0,
                waves_total: 5,
                started_tick: 1000,
                phase_started_tick: 1000,
                next_wave_tick: 1100,
                participants: vec![char_id.to_string()],
                failed: false,
            },
        ))
        .id();

    app.world_mut().entity_mut(entity).remove::<Client>();
    app.update();

    assert!(
        app.world().get::<TribulationState>(entity).is_none(),
        "期望 wave_current=0（劫刚触发）的 JueBi 断线也被结算，TribulationState 应被移除"
    );
    let fled: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<TribulationFled>>()
        .drain()
        .collect();
    assert_eq!(
        fled.len(),
        1,
        "期望劫初断线产生 1 个 TribulationFled，实际 {}",
        fled.len()
    );
    assert!(
        load_active_tribulation(&settings, char_id)
            .expect("查询应成功")
            .is_none(),
        "期望劫初断线后 active row 被删（无孤儿），实际行仍存在"
    );

    let _ = fs::remove_dir_all(root);
}
#[test]
fn juebi_disconnect_near_completion_is_fled() {
    let mut app = qi_test_app();
    let (settings, root) = persistence_settings("juebi-near-complete-fled");
    let char_id = "offline:NearWinner";
    let waves_total = 5_u32;
    let wave_current = waves_total - 1;
    persist_active_tribulation(
        &settings,
        &ActiveTribulationRecord {
            char_id: char_id.to_string(),
            kind: "jue_bi".to_string(),
            source: String::new(),
            origin_dimension: None,
            wave_current,
            waves_total,
            started_tick: 0,
            epicenter: [0.0, 64.0, 0.0],
            intensity: 0.0,
        },
    )
    .expect("active jue_bi 应能写入");

    app.insert_resource(settings.clone());
    app.insert_resource(CombatClock { tick: 9000 });
    app.add_event::<TribulationSettled>();
    app.add_event::<TribulationFled>();
    app.add_systems(Update, abort_du_xu_on_client_removed);

    let (mut client_bundle, _helper) = create_mock_client("NearWinner");
    client_bundle.player.position = Position::new([0.0, 64.0, 0.0]);
    let entity = app
        .world_mut()
        .spawn((
            client_bundle,
            Cultivation {
                realm: Realm::Spirit,
                qi_current: 50.0,
                qi_max: 210.0,
                ..Default::default()
            },
            all_meridians_open(),
            Wounds::default(),
            Lifecycle {
                character_id: char_id.to_string(),
                state: LifecycleState::Alive,
                ..Default::default()
            },
            LifeRecord::new(char_id),
            TribulationState {
                kind: TribulationKind::JueBi,
                phase: TribulationPhase::Wave(wave_current),
                epicenter: [0.0, 64.0, 0.0],
                wave_current,
                waves_total,
                started_tick: 0,
                phase_started_tick: 8900,
                next_wave_tick: 9200,
                participants: vec![char_id.to_string()],
                failed: false,
            },
        ))
        .id();

    app.world_mut().entity_mut(entity).remove::<Client>();
    app.update();

    assert!(
        app.world().get::<TribulationState>(entity).is_none(),
        "期望接近完成的 JueBi 断线被结算为 fled，TribulationState 应被移除"
    );
    let settled: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<TribulationSettled>>()
        .drain()
        .collect();
    assert_eq!(settled.len(), 1);
    assert_eq!(
        settled[0].result.waves_survived, wave_current,
        "期望 waves_survived = wave_current({wave_current})，实际 {}",
        settled[0].result.waves_survived
    );
    assert!(
        load_active_tribulation(&settings, char_id)
            .expect("查询应成功")
            .is_none(),
        "期望接近完成的 JueBi 断线后 active row 被删（无孤儿），实际行仍存在"
    );

    let _ = fs::remove_dir_all(root);
}
#[test]
fn leaving_lock_radius_flees_and_regresses_without_death() {
    let mut app = qi_test_app();
    let (settings, root) = persistence_settings("boundary-fled");
    let char_id = "offline:Azure";
    persist_active_tribulation(
        &settings,
        &ActiveTribulationRecord {
            char_id: char_id.to_string(),
            kind: "du_xu".to_string(),
            source: String::new(),
            origin_dimension: Some("minecraft:overworld".to_string()),
            wave_current: 1,
            waves_total: 3,
            started_tick: 80,
            epicenter: [0.0, 64.0, 0.0],
            intensity: 0.0,
        },
    )
    .expect("active tribulation should persist before flee");

    app.insert_resource(settings.clone());
    app.insert_resource(CombatClock { tick: 340 });
    app.add_event::<TribulationSettled>();
    app.add_event::<TribulationFled>();
    app.add_systems(Update, tribulation_escape_boundary_system);

    let entity = app
        .world_mut()
        .spawn((
            Position::new([30.0, 66.0, 0.0]),
            Cultivation {
                realm: Realm::Spirit,
                qi_current: 160.0,
                qi_max: 210.0,
                ..Default::default()
            },
            all_meridians_open(),
            Wounds {
                health_current: 0.0,
                health_max: 100.0,
                entries: Vec::new(),
            },
            Lifecycle {
                character_id: char_id.to_string(),
                state: LifecycleState::Alive,
                ..Default::default()
            },
            LifeRecord::new(char_id),
            TribulationState {
                kind: TribulationKind::DuXu,
                phase: TribulationPhase::Lock,
                epicenter: [0.0, 66.0, 0.0],
                wave_current: 1,
                waves_total: 3,
                started_tick: 80,
                phase_started_tick: 300,
                next_wave_tick: 360,
                participants: vec![char_id.to_string()],
                failed: false,
            },
        ))
        .id();

    app.update();

    assert!(app.world().get::<TribulationState>(entity).is_none());
    let cultivation = app
        .world()
        .get::<Cultivation>(entity)
        .expect("cultivation should remain attached");
    assert_eq!(cultivation.realm, Realm::Spirit);
    assert_eq!(cultivation.qi_current, 0.0);
    let life = app
        .world()
        .get::<LifeRecord>(entity)
        .expect("life record should remain attached");
    assert!(matches!(
        life.biography.last(),
        Some(BiographyEntry::TribulationFled { wave: 2, tick: 340 })
    ));

    let settled: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<TribulationSettled>>()
        .drain()
        .collect();
    assert_eq!(settled.len(), 1);
    assert_eq!(settled[0].result.outcome, DuXuOutcomeV1::Fled);
    assert_eq!(settled[0].result.waves_survived, 1);
    assert!(
        load_active_tribulation(&settings, char_id)
            .expect("active tribulation query should succeed")
            .is_none(),
        "fled tribulation should clear active row"
    );

    let _ = fs::remove_dir_all(root);
}
#[test]
fn changing_dimension_during_lock_flees_even_inside_radius() {
    let mut app = qi_test_app();
    let (settings, root) = persistence_settings("dimension-fled");
    let char_id = "offline:Azure";
    persist_active_tribulation(
        &settings,
        &ActiveTribulationRecord {
            char_id: char_id.to_string(),
            kind: "du_xu".to_string(),
            source: String::new(),
            origin_dimension: Some("minecraft:overworld".to_string()),
            wave_current: 1,
            waves_total: 3,
            started_tick: 80,
            epicenter: [0.0, 64.0, 0.0],
            intensity: 0.0,
        },
    )
    .expect("active tribulation should persist before flee");

    app.insert_resource(settings.clone());
    app.insert_resource(CombatClock { tick: 345 });
    app.add_event::<TribulationSettled>();
    app.add_event::<TribulationFled>();
    app.add_systems(Update, tribulation_escape_boundary_system);

    let entity = app
        .world_mut()
        .spawn((
            Position::new([0.0, 66.0, 0.0]),
            CurrentDimension(DimensionKind::Tsy),
            TribulationOriginDimension(DimensionKind::Overworld),
            Cultivation {
                realm: Realm::Spirit,
                qi_current: 160.0,
                qi_max: 210.0,
                ..Default::default()
            },
            all_meridians_open(),
            Wounds {
                health_current: 40.0,
                health_max: 100.0,
                entries: Vec::new(),
            },
            Lifecycle {
                character_id: char_id.to_string(),
                state: LifecycleState::Alive,
                ..Default::default()
            },
            LifeRecord::new(char_id),
            TribulationState {
                kind: TribulationKind::DuXu,
                phase: TribulationPhase::Lock,
                epicenter: [0.0, 66.0, 0.0],
                wave_current: 1,
                waves_total: 3,
                started_tick: 80,
                phase_started_tick: 300,
                next_wave_tick: 360,
                participants: vec![char_id.to_string()],
                failed: false,
            },
        ))
        .id();

    app.update();

    assert!(app.world().get::<TribulationState>(entity).is_none());
    assert!(app
        .world()
        .get::<TribulationOriginDimension>(entity)
        .is_none());
    let cultivation = app
        .world()
        .get::<Cultivation>(entity)
        .expect("cultivation should remain attached");
    assert_eq!(cultivation.realm, Realm::Spirit);
    assert_eq!(cultivation.qi_current, 0.0);
    let settled: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<TribulationSettled>>()
        .drain()
        .collect();
    assert_eq!(settled.len(), 1);
    assert_eq!(settled[0].result.outcome, DuXuOutcomeV1::Fled);
    let fled: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<TribulationFled>>()
        .drain()
        .collect();
    assert_eq!(fled.len(), 1);
    assert_eq!(fled[0].entity, entity);
    assert_eq!(fled[0].tick, 345);
    assert!(
        load_active_tribulation(&settings, char_id)
            .expect("active tribulation query should succeed")
            .is_none(),
        "dimension flee should clear active row"
    );

    let _ = fs::remove_dir_all(root);
}
#[test]
fn track_metrics_increments_halfstep_counter_per_event() {
    let mut app = p0_metrics_test_app();
    let dummy = app.world_mut().spawn(()).id();
    for _ in 0..10 {
        app.world_mut()
            .resource_mut::<Events<TribulationSettled>>()
            .send(make_settled_event(dummy, DuXuOutcomeV1::HalfStep));
    }
    app.update();
    let metrics = app.world().resource::<TribulationMetrics>();
    assert_eq!(
        metrics.halfstep_count, 10,
        "10 HalfStep settlements should increment counter to 10 (got {}); P0 验收 \
         — mock 10 halfstep 后 counter == 10",
        metrics.halfstep_count
    );
    assert_eq!(
        metrics.ascended_count, 0,
        "ascended_count must stay 0 when only HalfStep events fire; bleed-over bug"
    );
}
#[test]
fn track_metrics_increments_ascended_counter_per_event() {
    let mut app = p0_metrics_test_app();
    let dummy = app.world_mut().spawn(()).id();
    for _ in 0..5 {
        app.world_mut()
            .resource_mut::<Events<TribulationSettled>>()
            .send(make_settled_event(dummy, DuXuOutcomeV1::Ascended));
    }
    app.update();
    let metrics = app.world().resource::<TribulationMetrics>();
    assert_eq!(metrics.ascended_count, 5);
    assert_eq!(metrics.halfstep_count, 0);
}
#[test]
fn track_metrics_ignores_independent_juebi_halfstep_outcome() {
    // 化虚老怪扛过 VoidActionExplodeZone 触发的 JueBi 幸存 → outcome=HalfStep（schema 同），
    // 但语义不是"升格被 quota 拒"。绝不应授予半步 buff、不入队、不增 halfstep_count。
    let mut app = p0_metrics_test_app();
    let entity = app.world_mut().spawn(()).id();
    for _ in 0..10 {
        app.world_mut()
            .resource_mut::<Events<TribulationSettled>>()
            .send(make_settled_event_with_source(
                entity,
                DuXuOutcomeV1::HalfStep,
                JueBiTriggerSource::VoidActionExplodeZone,
            ));
    }
    app.update();
    let metrics = app.world().resource::<TribulationMetrics>();
    let queue = app.world().resource::<HalfStepRechallengeQueue>();
    assert_eq!(
        metrics.halfstep_count, 0,
        "VoidActionExplodeZone 路径的 HalfStep outcome 不应增 halfstep_count；\
         否则化虚老怪频繁触发非升格 JueBi 会污染遥测"
    );
    assert!(
        queue.is_empty(),
        "独立 JueBi 幸存者不应入重渡队列；否则化虚老怪会被列为半步等待升格名额"
    );
    assert!(
        app.world().get::<HalfStepState>(entity).is_none(),
        "独立 JueBi 幸存者不应被插 HalfStepState component；否则会被误授半步 buff"
    );
}
#[test]
fn track_metrics_ignores_failed_killed_fled_outcomes() {
    let mut app = p0_metrics_test_app();
    let dummy = app.world_mut().spawn(()).id();
    for outcome in [
        DuXuOutcomeV1::Killed,
        DuXuOutcomeV1::Failed,
        DuXuOutcomeV1::Fled,
    ] {
        app.world_mut()
            .resource_mut::<Events<TribulationSettled>>()
            .send(make_settled_event(dummy, outcome));
    }
    app.update();
    let metrics = app.world().resource::<TribulationMetrics>();
    assert_eq!(
        metrics.halfstep_count, 0,
        "Killed/Failed/Fled outcomes must not touch halfstep counter"
    );
    assert_eq!(metrics.ascended_count, 0);
}
#[test]
fn track_metrics_inserts_halfstep_state_with_correct_entered_at() {
    let mut app = p0_metrics_test_app();
    app.world_mut().resource_mut::<CombatClock>().tick = 12345;
    let target = app.world_mut().spawn(()).id();
    app.world_mut()
        .resource_mut::<Events<TribulationSettled>>()
        .send(make_settled_event(target, DuXuOutcomeV1::HalfStep));
    app.update();
    let state = app
        .world()
        .get::<HalfStepState>(target)
        .copied()
        .expect("HalfStepState component must be inserted on HalfStep settlement");
    assert_eq!(state.entered_at, 12345, "entered_at must equal clock.tick");
    assert_eq!(
        state.rechallenge_window_until,
        12345 + RECHALLENGE_WINDOW_TICKS,
        "rechallenge_window_until must be entered_at + 7d (§8 Q1)"
    );
    assert!(
        !state.buff_applied,
        "buff_applied starts false (§8 Q4 守卫)"
    );
}
#[test]
fn track_metrics_does_not_insert_halfstep_state_for_non_halfstep_outcomes() {
    let mut app = p0_metrics_test_app();
    let target = app.world_mut().spawn(()).id();
    app.world_mut()
        .resource_mut::<Events<TribulationSettled>>()
        .send(make_settled_event(target, DuXuOutcomeV1::Ascended));
    app.update();
    assert!(
        app.world().get::<HalfStepState>(target).is_none(),
        "Ascended outcome must NOT insert HalfStepState; bleed-over would put 化虚 修士 in 半步 queue"
    );
}
#[test]
fn quota_full_tracker_starts_full_period_on_occupied_event() {
    let mut app = p0_metrics_test_app();
    // limit = floor(100 / DEFAULT_VOID_QUOTA_K=50) = 2，先令 occupied 达到 limit
    app.world_mut().resource_mut::<CombatClock>().tick = 100;
    app.world_mut()
        .resource_mut::<Events<AscensionQuotaOccupied>>()
        .send(AscensionQuotaOccupied { occupied_slots: 2 });
    app.update();
    let tracker = app.world().resource::<QuotaFullTracker>();
    assert_eq!(tracker.current_occupied, 2);
    assert_eq!(
        tracker.current_limit, 2,
        "limit should be derived from check_void_quota(WorldQiBudget=100, k=50)"
    );
    assert_eq!(
        tracker.full_since_tick,
        Some(100),
        "进入 full 状态应记录 full_since_tick=current clock"
    );
}
#[test]
fn quota_full_tracker_ends_period_and_accumulates_on_opened_event() {
    let mut app = p0_metrics_test_app();
    app.world_mut().resource_mut::<CombatClock>().tick = 100;
    app.world_mut()
        .resource_mut::<Events<AscensionQuotaOccupied>>()
        .send(AscensionQuotaOccupied { occupied_slots: 2 });
    app.update();
    // 时间推进 500 ticks，quota 名额空出
    app.world_mut().resource_mut::<CombatClock>().tick = 600;
    app.world_mut()
        .resource_mut::<Events<AscensionQuotaOpened>>()
        .send(AscensionQuotaOpened { occupied_slots: 1 });
    app.update();
    let metrics = app.world().resource::<TribulationMetrics>();
    let tracker = app.world().resource::<QuotaFullTracker>();
    assert_eq!(
        metrics.quota_full_duration_ticks, 500,
        "离开 full 状态后应把 (600-100)=500 写入累计计数；off-by-one 或漏算"
    );
    assert!(
        tracker.full_since_tick.is_none(),
        "离开 full 状态后 full_since_tick 必须清空"
    );
}
#[test]
fn current_quota_full_duration_includes_pending_window() {
    let mut app = p0_metrics_test_app();
    app.world_mut().resource_mut::<CombatClock>().tick = 100;
    app.world_mut()
        .resource_mut::<Events<AscensionQuotaOccupied>>()
        .send(AscensionQuotaOccupied { occupied_slots: 2 });
    app.update();
    let metrics = *app.world().resource::<TribulationMetrics>();
    let tracker = *app.world().resource::<QuotaFullTracker>();
    // 当前仍在 full 状态：current_tick=200，pending = 200 - 100 = 100
    let observed = current_quota_full_duration_ticks(&metrics, &tracker, 200);
    assert_eq!(
        observed, 100,
        "current 函数应把 pending (current_tick - full_since_tick) 加到 base 上"
    );
}
#[test]
fn halfstep_state_is_within_window_boundary() {
    let state = HalfStepState::new(1000);
    let window_end = 1000 + RECHALLENGE_WINDOW_TICKS;
    assert!(state.is_within_window(1000), "进入 tick 应在窗口内");
    assert!(
        state.is_within_window(window_end),
        "等于窗口末端应在窗口内（闭区间）"
    );
    assert!(
        !state.is_within_window(window_end + 1),
        "超过窗口末端 1 tick 必须不在窗口内（边界一致性）"
    );
}
#[test]
fn halfstep_buff_applies_qi_max_and_lifespan_on_first_settlement() {
    let mut app = p0_metrics_test_app();
    let entity = spawn_halfstep_candidate(&mut app, 1000.0, LifespanCapTable::SPIRIT);
    app.world_mut()
        .resource_mut::<Events<TribulationSettled>>()
        .send(make_settled_event(entity, DuXuOutcomeV1::HalfStep));
    app.update();

    let cultivation = app.world().get::<Cultivation>(entity).expect("cultivation");
    let lifespan = app
        .world()
        .get::<LifespanComponent>(entity)
        .expect("lifespan");
    let state = app
        .world()
        .get::<HalfStepState>(entity)
        .expect("halfstep state");

    // qi_max × (1.0 + 0.10) = 1100.0
    let expected_qi_max = 1000.0 * (1.0 + HALFSTEP_QI_MAX_BONUS as f64);
    assert!(
        (cultivation.qi_max - expected_qi_max).abs() < 1e-6,
        "qi_max expected {expected_qi_max} but got {} (HALFSTEP_QI_MAX_BONUS={HALFSTEP_QI_MAX_BONUS})",
        cultivation.qi_max
    );
    assert_eq!(
        lifespan.cap_by_realm,
        LifespanCapTable::SPIRIT + HALFSTEP_LIFESPAN_BONUS_YEARS,
        "lifespan cap should increment by HALFSTEP_LIFESPAN_BONUS_YEARS={HALFSTEP_LIFESPAN_BONUS_YEARS}"
    );
    assert!(
        state.buff_applied,
        "buff_applied must be true after first HalfStep settlement on entity with Cultivation"
    );
}
#[test]
fn halfstep_buff_emits_audit_qi_transfer_event() {
    let mut app = p0_metrics_test_app();
    let entity = spawn_halfstep_candidate(&mut app, 1000.0, LifespanCapTable::SPIRIT);
    app.world_mut()
        .resource_mut::<Events<TribulationSettled>>()
        .send(make_settled_event(entity, DuXuOutcomeV1::HalfStep));
    app.update();
    let transfers = collect_qi_transfers(&mut app);

    let halfstep_transfers: Vec<&QiTransfer> = transfers
        .iter()
        .filter(|t| matches!(t.reason, QiTransferReason::HalfStepBuff))
        .collect();
    assert_eq!(
        halfstep_transfers.len(),
        1,
        "expected exactly 1 HalfStepBuff QiTransfer event (got {}); transfers={transfers:?}",
        halfstep_transfers.len()
    );
    let transfer = halfstep_transfers[0];
    // bonus = 1000 × 0.10 = 100
    let expected_bonus = 1000.0 * HALFSTEP_QI_MAX_BONUS as f64;
    assert!(
        (transfer.amount - expected_bonus).abs() < 1e-6,
        "amount expected {expected_bonus} got {}",
        transfer.amount
    );
    assert_eq!(transfer.from, QiAccountId::tiandao());
    assert_eq!(transfer.to, QiAccountId::player("halfstep_test_char"));
}
#[test]
fn halfstep_buff_not_reapplied_when_state_already_marks_buff_applied() {
    let mut app = p0_metrics_test_app();
    let entity = spawn_halfstep_candidate(&mut app, 1000.0, LifespanCapTable::SPIRIT);
    // Pre-insert HalfStepState 标记 buff_applied=true（模拟二次进入 HalfStep 的状态）
    app.world_mut().entity_mut(entity).insert(HalfStepState {
        entered_at: 100,
        rechallenge_window_until: 100 + RECHALLENGE_WINDOW_TICKS,
        buff_applied: true,
    });
    // 触发新一次 HalfStep settlement
    app.world_mut()
        .resource_mut::<Events<TribulationSettled>>()
        .send(make_settled_event(entity, DuXuOutcomeV1::HalfStep));
    app.update();

    let cultivation = app.world().get::<Cultivation>(entity).unwrap();
    let lifespan = app.world().get::<LifespanComponent>(entity).unwrap();
    let state = app.world().get::<HalfStepState>(entity).unwrap();
    assert_eq!(
        cultivation.qi_max, 1000.0,
        "qi_max must stay 1000.0 (not re-buffed); reapply would yield 1100.0 — §8 Q4 守卫失效"
    );
    assert_eq!(
        lifespan.cap_by_realm,
        LifespanCapTable::SPIRIT,
        "lifespan cap must stay at SPIRIT base; reapply守卫失效"
    );
    assert_eq!(
        state.entered_at, 100,
        "entered_at must preserve original value (rechallenge window 起点稳定)"
    );
    let transfers = collect_qi_transfers(&mut app);
    let halfstep_transfers: Vec<&QiTransfer> = transfers
        .iter()
        .filter(|t| matches!(t.reason, QiTransferReason::HalfStepBuff))
        .collect();
    assert!(
        halfstep_transfers.is_empty(),
        "no ledger emit on reapply skip; got {halfstep_transfers:?}"
    );
    // metric 仍累计（halfstep_count 计的是 settlement 次数，不是 buff 次数）
    assert_eq!(
        app.world().resource::<TribulationMetrics>().halfstep_count,
        1,
        "halfstep_count tracks settlement events, not unique entities; should still increment"
    );
}
#[test]
fn halfstep_buff_applies_to_qi_max_only_when_lifespan_component_absent() {
    let mut app = p0_metrics_test_app();
    let entity = app
        .world_mut()
        .spawn(Cultivation {
            realm: Realm::Spirit,
            qi_current: 0.0,
            qi_max: 500.0,
            ..Default::default()
        })
        .id();
    app.world_mut()
        .resource_mut::<Events<TribulationSettled>>()
        .send(make_settled_event(entity, DuXuOutcomeV1::HalfStep));
    app.update();
    let cultivation = app.world().get::<Cultivation>(entity).unwrap();
    let expected = 500.0 * (1.0 + HALFSTEP_QI_MAX_BONUS as f64);
    assert!(
        (cultivation.qi_max - expected).abs() < 1e-6,
        "qi_max should still buff even without lifespan; got {}",
        cultivation.qi_max
    );
    let state = app.world().get::<HalfStepState>(entity).unwrap();
    assert!(state.buff_applied);
}
#[test]
fn halfstep_buff_skipped_when_entity_lacks_cultivation_and_state_left_unbuffed() {
    let mut app = p0_metrics_test_app();
    // 仅 bare entity（无 Cultivation），模拟 dormant NPC / 测试 stub
    let entity = app.world_mut().spawn(()).id();
    app.world_mut()
        .resource_mut::<Events<TribulationSettled>>()
        .send(make_settled_event(entity, DuXuOutcomeV1::HalfStep));
    app.update();

    let state = app
        .world()
        .get::<HalfStepState>(entity)
        .expect("HalfStepState 仍应插入（P3 重渡队列需要）");
    assert!(
        !state.buff_applied,
        "无 Cultivation entity 应留 buff_applied=false，等待 hydrate 后由后续 settlement 应用；\
         直接置 true 会丢失「未来需补 buff」信号"
    );
    // ledger 不应 emit
    let transfers = collect_qi_transfers(&mut app);
    assert!(
        transfers
            .iter()
            .all(|t| !matches!(t.reason, QiTransferReason::HalfStepBuff)),
        "无 Cultivation 路径不应触发 HalfStepBuff 转账事件"
    );
}
#[test]
fn rechallenge_queue_enqueue_preserves_fcfs_by_entered_at() {
    let mut queue = HalfStepRechallengeQueue::default();
    // 乱序入队
    for (cid, et) in [("late", 300), ("early", 100), ("mid", 200)] {
        queue.enqueue(HalfStepRechallengeEntry {
            char_id: cid.to_string(),
            entity: Entity::from_raw(1),
            entered_at: et,
            rechallenge_window_until: et + RECHALLENGE_WINDOW_TICKS,
            is_dormant: false,
            buff_applied: false,
        });
    }
    let order: Vec<&str> = queue.queue.iter().map(|e| e.char_id.as_str()).collect();
    assert_eq!(
        order,
        vec!["early", "mid", "late"],
        "enqueue 必须按 entered_at 升序插入，FCFS 公平性 (§8 Q2)"
    );
}
#[test]
fn rechallenge_queue_remove_entity_clears_all_entries_for_target() {
    let mut queue = HalfStepRechallengeQueue::default();
    let target = Entity::from_raw(7);
    let other = Entity::from_raw(8);
    queue.enqueue(HalfStepRechallengeEntry {
        char_id: "a".to_string(),
        entity: target,
        entered_at: 100,
        rechallenge_window_until: 100 + RECHALLENGE_WINDOW_TICKS,
        is_dormant: false,
        buff_applied: false,
    });
    queue.enqueue(HalfStepRechallengeEntry {
        char_id: "b".to_string(),
        entity: other,
        entered_at: 200,
        rechallenge_window_until: 200 + RECHALLENGE_WINDOW_TICKS,
        is_dormant: false,
        buff_applied: false,
    });
    queue.remove_entity(target);
    assert_eq!(queue.len(), 1, "目标 entity 应已移除");
    assert_eq!(queue.queue[0].entity, other);
}
#[test]
fn settlement_enqueues_halfstep_entity_on_first_settle() {
    let mut app = p0_metrics_test_app();
    app.world_mut().resource_mut::<CombatClock>().tick = 555;
    // 用 hydrated entity（带 Cultivation）测 is_dormant=false 分支
    let entity = spawn_halfstep_candidate(&mut app, 100.0, LifespanCapTable::SPIRIT);
    app.world_mut()
        .resource_mut::<Events<TribulationSettled>>()
        .send(make_settled_event(entity, DuXuOutcomeV1::HalfStep));
    app.update();
    let queue = app.world().resource::<HalfStepRechallengeQueue>();
    assert_eq!(queue.len(), 1, "首次 HalfStep settlement 必入队");
    let entry = &queue.queue[0];
    assert_eq!(entry.char_id, "halfstep_test_char");
    assert_eq!(entry.entity, entity);
    assert_eq!(entry.entered_at, 555);
    assert_eq!(
        entry.rechallenge_window_until,
        555 + RECHALLENGE_WINDOW_TICKS
    );
    assert!(
        !entry.is_dormant,
        "hydrated entity（带 Cultivation）必须 is_dormant=false"
    );
}
#[test]
fn settlement_does_not_double_enqueue_on_repeat_halfstep() {
    let mut app = p0_metrics_test_app();
    let entity = app.world_mut().spawn(()).id();
    // 第一次 HalfStep
    app.world_mut()
        .resource_mut::<Events<TribulationSettled>>()
        .send(make_settled_event(entity, DuXuOutcomeV1::HalfStep));
    app.update();
    // 第二次 HalfStep（已 has_state，应跳过入队）
    app.world_mut()
        .resource_mut::<Events<TribulationSettled>>()
        .send(make_settled_event(entity, DuXuOutcomeV1::HalfStep));
    app.update();
    let queue = app.world().resource::<HalfStepRechallengeQueue>();
    assert_eq!(
        queue.len(),
        1,
        "二次 HalfStep settlement 必须不入队（has_state=true 守卫）"
    );
}
#[test]
fn settlement_removes_from_queue_and_state_on_ascended_killed_failed_fled() {
    for outcome in [
        DuXuOutcomeV1::Ascended,
        DuXuOutcomeV1::Killed,
        DuXuOutcomeV1::Failed,
        DuXuOutcomeV1::Fled,
    ] {
        let mut app = p0_metrics_test_app();
        let entity = app.world_mut().spawn(()).id();
        // 先制造一个 HalfStep 入队
        app.world_mut()
            .resource_mut::<Events<TribulationSettled>>()
            .send(make_settled_event(entity, DuXuOutcomeV1::HalfStep));
        app.update();
        assert_eq!(app.world().resource::<HalfStepRechallengeQueue>().len(), 1);
        assert!(
            app.world().get::<HalfStepState>(entity).is_some(),
            "HalfStep settle 后必须有 HalfStepState component（前提）"
        );
        // 后续 outcome 触发清队 + 清 component
        app.world_mut()
            .resource_mut::<Events<TribulationSettled>>()
            .send(make_settled_event(entity, outcome));
        app.update();
        assert_eq!(
            app.world().resource::<HalfStepRechallengeQueue>().len(),
            0,
            "outcome={outcome:?} 应触发清队"
        );
        assert!(
            app.world().get::<HalfStepState>(entity).is_none(),
            "outcome={outcome:?} 必须 remove HalfStepState component；\
             留 stale state 会污染下一轮 HalfStep（entered_at 错乱 / buff_applied 守卫失效）"
        );
    }
}
#[test]
fn halfstep_re_settle_after_dispatch_pop_re_enqueues_and_preserves_entered_at() {
    // 场景：玩家 HalfStep settle → 入队。AscensionQuotaOpened 触发 dispatch pop_front。
    // 玩家未起劫，又一次 HalfStep settle 进来 —— 必须**重新入队**（否则永久失去 FCFS 资格）
    // 且 entered_at **不变**（§8 Q1 重渡窗口起点稳定）。
    let mut app = p0_metrics_test_app();
    app.world_mut().resource_mut::<CombatClock>().tick = 100;
    let entity = app.world_mut().spawn(()).id();
    app.world_mut()
        .resource_mut::<Events<TribulationSettled>>()
        .send(make_settled_event(entity, DuXuOutcomeV1::HalfStep));
    app.update();
    // dispatch 弹出
    app.world_mut()
        .resource_mut::<Events<AscensionQuotaOpened>>()
        .send(AscensionQuotaOpened { occupied_slots: 0 });
    app.update();
    assert!(
        app.world()
            .resource::<HalfStepRechallengeQueue>()
            .is_empty(),
        "dispatch 应已弹出该 entry"
    );
    let original_entered_at = app
        .world()
        .get::<HalfStepState>(entity)
        .map(|s| s.entered_at)
        .expect("HalfStepState 仍应在 component 上（dispatch 不清 component）");
    // 时间推进 + 二次 HalfStep
    app.world_mut().resource_mut::<CombatClock>().tick = 500;
    app.world_mut()
        .resource_mut::<Events<TribulationSettled>>()
        .send(make_settled_event(entity, DuXuOutcomeV1::HalfStep));
    app.update();
    assert_eq!(
        app.world().resource::<HalfStepRechallengeQueue>().len(),
        1,
        "二次 HalfStep settle 必须重新入队；否则玩家永久失去重渡资格"
    );
    let state_after = app
        .world()
        .get::<HalfStepState>(entity)
        .expect("HalfStepState 仍在 component");
    assert_eq!(
        state_after.entered_at, original_entered_at,
        "entered_at 必须保持原值（{original_entered_at}）；重置会让 7d 窗口被玩家无限延期"
    );
}
#[test]
fn halfstep_buff_applied_flag_backfills_on_resettle_when_cultivation_now_present() {
    // 场景：dormant NPC 第一次 HalfStep（缺 Cultivation → buff_applied=false）。
    // 后续 hydrate 后该 entity 有 Cultivation 了，再次 HalfStep settle 时 buff 应回写应用。
    let mut app = p0_metrics_test_app();
    let entity = app.world_mut().spawn(()).id();
    // Phase 1: dormant - 无 Cultivation
    app.world_mut()
        .resource_mut::<Events<TribulationSettled>>()
        .send(make_settled_event(entity, DuXuOutcomeV1::HalfStep));
    app.update();
    let state_before = app.world().get::<HalfStepState>(entity).expect("state");
    assert!(
        !state_before.buff_applied,
        "无 Cultivation 路径下 buff_applied 必须留 false（避免谎报应用过）"
    );
    let entered_at_before = state_before.entered_at;

    // Phase 2: hydrate - 给 entity 加 Cultivation + Lifespan
    app.world_mut().entity_mut(entity).insert((
        Cultivation {
            realm: Realm::Spirit,
            qi_current: 0.0,
            qi_max: 1000.0,
            ..Default::default()
        },
        LifespanComponent::new(LifespanCapTable::SPIRIT),
    ));
    // 二次 HalfStep settle
    app.world_mut()
        .resource_mut::<Events<TribulationSettled>>()
        .send(make_settled_event(entity, DuXuOutcomeV1::HalfStep));
    app.update();

    let cultivation = app.world().get::<Cultivation>(entity).unwrap();
    let lifespan = app.world().get::<LifespanComponent>(entity).unwrap();
    let state_after = app.world().get::<HalfStepState>(entity).unwrap();
    let expected_qi_max = 1000.0 * (1.0 + HALFSTEP_QI_MAX_BONUS as f64);
    assert!(
        (cultivation.qi_max - expected_qi_max).abs() < 1e-6,
        "hydrate 后二次 settle 必须补 buff (qi_max {} → expected {})",
        cultivation.qi_max,
        expected_qi_max
    );
    assert_eq!(
        lifespan.cap_by_realm,
        LifespanCapTable::SPIRIT + HALFSTEP_LIFESPAN_BONUS_YEARS,
        "lifespan 也必须补"
    );
    assert!(state_after.buff_applied, "buff_applied 必须回写 true");
    assert_eq!(
        state_after.entered_at, entered_at_before,
        "entered_at 必须保留原值（dormant 阶段进 HalfStep 的时间戳）"
    );
}
#[test]
fn halfstep_buff_does_not_double_apply_within_same_frame() {
    // CodeRabbit P4 review #1：同帧多条 HalfStep event 给同一 entity →
    // commands.insert 是 deferred，第二条事件读 existing_states 仍是 None →
    // 原代码会把 buff 应用两次（破 §8 Q4）。
    // 修复后：staged_states local map 镜像 commands 队列，第二条事件读到 staged 的
    // buff_applied=true，跳过第二次应用。
    let mut app = p0_metrics_test_app();
    let entity = spawn_halfstep_candidate(&mut app, 1000.0, LifespanCapTable::SPIRIT);
    // 一次 update 内 dispatch 两条 HalfStep event 给同一 entity
    for _ in 0..2 {
        app.world_mut()
            .resource_mut::<Events<TribulationSettled>>()
            .send(make_settled_event(entity, DuXuOutcomeV1::HalfStep));
    }
    app.update();

    let cultivation = app.world().get::<Cultivation>(entity).unwrap();
    let lifespan = app.world().get::<LifespanComponent>(entity).unwrap();
    let expected_qi_max = 1000.0 * (1.0 + HALFSTEP_QI_MAX_BONUS as f64);
    assert!(
        (cultivation.qi_max - expected_qi_max).abs() < 1e-6,
        "同帧二次 HalfStep 不应重复 buff；expected {expected_qi_max} got {} (重复=1210)",
        cultivation.qi_max
    );
    assert_eq!(
        lifespan.cap_by_realm,
        LifespanCapTable::SPIRIT + HALFSTEP_LIFESPAN_BONUS_YEARS,
        "lifespan 同帧也不该重复 +200"
    );
    let queue = app.world().resource::<HalfStepRechallengeQueue>();
    assert_eq!(
        queue.len(),
        1,
        "队列只该有 1 条（dedup-then-enqueue）；got {}",
        queue.len()
    );
}
#[test]
fn enqueue_preserves_entered_at_when_entity_swaps_under_same_char_id() {
    // P4 review #2：dormant→hydrate 换 ECS entity 时，新 entity 必须复用旧 entry 的
    // entered_at + rechallenge_window_until（按 char_id 查队列），否则 7d 窗口被刷新
    // + FCFS 顺序乱（玩家可以通过反复重连无限延长窗口）
    let mut app = p0_metrics_test_app();
    app.world_mut().resource_mut::<CombatClock>().tick = 100;
    let entity_a = app.world_mut().spawn(()).id();
    let mut ev_a = make_settled_event_with_source(
        entity_a,
        DuXuOutcomeV1::HalfStep,
        JueBiTriggerSource::VoidQuotaExceeded,
    );
    ev_a.result.char_id = "swap_char".to_string();
    app.world_mut()
        .resource_mut::<Events<TribulationSettled>>()
        .send(ev_a);
    app.update();
    let original_entered_at =
        app.world().resource::<HalfStepRechallengeQueue>().queue[0].entered_at;
    assert_eq!(original_entered_at, 100, "前提：原 entered_at=100");

    // 模拟 dormant→hydrate：entity_a 被 despawn 或换号；新 entity_b 用同 char_id 再次结算
    // 推进 tick 模拟跨帧重 spawn
    app.world_mut().resource_mut::<CombatClock>().tick = 500;
    let entity_b = app.world_mut().spawn(()).id();
    assert_ne!(entity_a, entity_b);
    let mut ev_b = make_settled_event_with_source(
        entity_b,
        DuXuOutcomeV1::HalfStep,
        JueBiTriggerSource::VoidQuotaExceeded,
    );
    ev_b.result.char_id = "swap_char".to_string();
    app.world_mut()
        .resource_mut::<Events<TribulationSettled>>()
        .send(ev_b);
    app.update();

    let queue = app.world().resource::<HalfStepRechallengeQueue>();
    assert_eq!(queue.len(), 1, "char_id dedup 后只剩 1 条");
    let entry = &queue.queue[0];
    assert_eq!(
        entry.entered_at, original_entered_at,
        "新 entity 必须复用旧 entered_at={original_entered_at}（不能用 clock.tick=500 重建）；\
         否则 §8 Q1 7d 窗口被刷新"
    );
    assert_eq!(
        entry.rechallenge_window_until,
        original_entered_at + RECHALLENGE_WINDOW_TICKS,
        "rechallenge_window_until 必须从原 entered_at 推算（不漂移）"
    );
    assert_eq!(entry.entity, entity_b, "entity 字段更新为最新 ECS 句柄");
}
#[test]
fn entity_swap_under_same_char_id_inherits_buff_applied_and_does_not_double_buff() {
    // P5 review #1 真 bug：dormant→hydrate 换 entity 后 char_id-fallback 复用窗口，
    // 但若 buff_applied 没继承，新 entity 会被认为"全新"再加一次 buff，破 §8 Q4
    let mut app = p0_metrics_test_app();
    app.world_mut().resource_mut::<CombatClock>().tick = 100;
    // entity_a: hydrated（带 Cultivation），首次 HalfStep → 应用 buff
    let entity_a = spawn_halfstep_candidate(&mut app, 1000.0, LifespanCapTable::SPIRIT);
    let mut ev_a = make_settled_event_with_source(
        entity_a,
        DuXuOutcomeV1::HalfStep,
        JueBiTriggerSource::VoidQuotaExceeded,
    );
    ev_a.result.char_id = "char_with_buff".to_string();
    app.world_mut()
        .resource_mut::<Events<TribulationSettled>>()
        .send(ev_a);
    app.update();
    // 验证 entity_a 已 buff
    let cultivation_a = app.world().get::<Cultivation>(entity_a).unwrap();
    let expected_after_first_buff = 1000.0 * (1.0 + HALFSTEP_QI_MAX_BONUS as f64);
    assert!(
        (cultivation_a.qi_max - expected_after_first_buff).abs() < 1e-6,
        "前提：entity_a qi_max 应已 buff 到 1100；got {}",
        cultivation_a.qi_max
    );
    let entry_buff_flag = app.world().resource::<HalfStepRechallengeQueue>().queue[0].buff_applied;
    assert!(
        entry_buff_flag,
        "前提：队列 entry.buff_applied 应为 true（镜像 HalfStepState.buff_applied）"
    );

    // Despawn entity_a（模拟 dormant），新 entity_b 用同 char_id 再 settle
    app.world_mut().entity_mut(entity_a).despawn();
    app.world_mut().resource_mut::<CombatClock>().tick = 500;
    let entity_b = spawn_halfstep_candidate(&mut app, 2000.0, LifespanCapTable::SPIRIT);
    let entity_b_qi_max_before = app.world().get::<Cultivation>(entity_b).unwrap().qi_max;
    assert_eq!(
        entity_b_qi_max_before, 2000.0,
        "前提：entity_b qi_max 起始 2000（与 entity_a 区分以验证不被错误改）"
    );
    let mut ev_b = make_settled_event_with_source(
        entity_b,
        DuXuOutcomeV1::HalfStep,
        JueBiTriggerSource::VoidQuotaExceeded,
    );
    ev_b.result.char_id = "char_with_buff".to_string();
    app.world_mut()
        .resource_mut::<Events<TribulationSettled>>()
        .send(ev_b);
    app.update();

    // 核心断言：entity_b qi_max 不变（不被再 buff），尽管它确实 settle 了 HalfStep
    let cultivation_b = app.world().get::<Cultivation>(entity_b).unwrap();
    assert_eq!(
        cultivation_b.qi_max, 2000.0,
        "char_id-fallback 继承 buff_applied=true → 不重复 buff entity_b；\
         如果错误地重复 buff 会得到 2200（破 §8 Q4 不叠加守卫）"
    );
    // HalfStepState 也必须显示 buff_applied=true
    let state_b = app.world().get::<HalfStepState>(entity_b).unwrap();
    assert!(
        state_b.buff_applied,
        "entity_b 的 HalfStepState.buff_applied 必须继承 true，否则 deletion+respawn 又破守卫"
    );
}
#[test]
fn enqueue_marks_dormant_when_entity_lacks_cultivation() {
    // P4 review #1：缺 Cultivation 的 entity 是 dormant 占位（参 plan-npc-virtualize-v1
    // 二态 MVP），入队时必须标记 is_dormant=true，让 dispatch trigger 接收方
    // 区分在线/休眠目标
    let mut app = p0_metrics_test_app();
    let dormant = app.world_mut().spawn(()).id();
    app.world_mut()
        .resource_mut::<Events<TribulationSettled>>()
        .send(make_settled_event(dormant, DuXuOutcomeV1::HalfStep));
    app.update();
    let queue = app.world().resource::<HalfStepRechallengeQueue>();
    assert_eq!(queue.len(), 1);
    assert!(
        queue.queue[0].is_dormant,
        "缺 Cultivation entity 必须标 is_dormant=true；hydrate-on-trigger 路径靠此识别"
    );
}
#[test]
fn enqueue_marks_not_dormant_when_entity_has_cultivation() {
    // 镜像测试：hydrated entity（带 Cultivation）必须 is_dormant=false
    let mut app = p0_metrics_test_app();
    let hydrated = spawn_halfstep_candidate(&mut app, 1000.0, LifespanCapTable::SPIRIT);
    app.world_mut()
        .resource_mut::<Events<TribulationSettled>>()
        .send(make_settled_event(hydrated, DuXuOutcomeV1::HalfStep));
    app.update();
    let queue = app.world().resource::<HalfStepRechallengeQueue>();
    assert!(
        !queue.queue[0].is_dormant,
        "带 Cultivation 的 hydrated entity 必须 is_dormant=false"
    );
}
#[test]
fn enqueue_dedupes_same_char_id_across_different_entities() {
    // CodeRabbit P4 review #4：dormant→hydrate 之间换了 ECS entity 但 char_id 持久；
    // 入队前的 char_id dedup 必须把 entity A 的旧 entry 清掉，最终队列只剩 entity B
    let mut app = p0_metrics_test_app();
    app.world_mut().resource_mut::<CombatClock>().tick = 100;
    // entity A 先用 char_id="X" 入队
    let entity_a = app.world_mut().spawn(()).id();
    let mut ev_a = make_settled_event_with_source(
        entity_a,
        DuXuOutcomeV1::HalfStep,
        JueBiTriggerSource::VoidQuotaExceeded,
    );
    ev_a.result.char_id = "shared_char_id".to_string();
    app.world_mut()
        .resource_mut::<Events<TribulationSettled>>()
        .send(ev_a);
    app.update();
    assert_eq!(app.world().resource::<HalfStepRechallengeQueue>().len(), 1);

    // 时间推进；entity B（不同 ECS 句柄，模拟 hydrate 重 spawn）用同 char_id 再次 HalfStep
    app.world_mut().resource_mut::<CombatClock>().tick = 500;
    let entity_b = app.world_mut().spawn(()).id();
    assert_ne!(entity_a, entity_b, "测试前提：entity A != B");
    let mut ev_b = make_settled_event_with_source(
        entity_b,
        DuXuOutcomeV1::HalfStep,
        JueBiTriggerSource::VoidQuotaExceeded,
    );
    ev_b.result.char_id = "shared_char_id".to_string();
    app.world_mut()
        .resource_mut::<Events<TribulationSettled>>()
        .send(ev_b);
    app.update();

    let queue = app.world().resource::<HalfStepRechallengeQueue>();
    assert_eq!(
        queue.len(),
        1,
        "char_id dedup 后队列只剩 1 条；同 char_id 双占 FIFO 会让玩家收到重复 trigger"
    );
    let entry = &queue.queue[0];
    assert_eq!(
        entry.entity, entity_b,
        "残留 entry 必须是后入队的 entity B（旧 entity A entry 已被 remove_char_id 清除）"
    );
    assert_eq!(entry.char_id, "shared_char_id");
}
#[test]
fn dispatch_rechallenge_emits_trigger_for_queue_head_on_quota_opened() {
    let mut app = p0_metrics_test_app();
    app.world_mut().resource_mut::<CombatClock>().tick = 1000;
    // 用 hydrated entity（带 Cultivation）验证 is_dormant=false 的派发路径
    let entity = spawn_halfstep_candidate(&mut app, 100.0, LifespanCapTable::SPIRIT);
    app.world_mut()
        .resource_mut::<Events<TribulationSettled>>()
        .send(make_settled_event(entity, DuXuOutcomeV1::HalfStep));
    app.update();
    // 触发名额空出
    app.world_mut()
        .resource_mut::<Events<AscensionQuotaOpened>>()
        .send(AscensionQuotaOpened { occupied_slots: 1 });
    app.update();
    let triggers = drain_rechallenge_triggers(&mut app);
    assert_eq!(
        triggers.len(),
        1,
        "AscensionQuotaOpened 应派发一个 rechallenge trigger; got {}",
        triggers.len()
    );
    assert_eq!(triggers[0].entity, entity);
    assert_eq!(triggers[0].char_id, "halfstep_test_char");
    assert!(
        !triggers[0].is_dormant,
        "hydrated entity 派发的 trigger 必须 is_dormant=false"
    );
    // 队列已出队
    assert!(app
        .world()
        .resource::<HalfStepRechallengeQueue>()
        .is_empty());
}
#[test]
fn dispatch_drops_expired_entries_and_continues_to_next() {
    let mut app = p0_metrics_test_app();
    app.world_mut().resource_mut::<CombatClock>().tick = 100;
    // stale 和 fresh 必须用不同 char_id，否则 fresh 会继承 stale 的窗口
    // （P4 review #2 char_id-fallback 复用窗口的预期行为）
    let stale = app.world_mut().spawn(()).id();
    let mut ev_stale = make_settled_event_with_source(
        stale,
        DuXuOutcomeV1::HalfStep,
        JueBiTriggerSource::VoidQuotaExceeded,
    );
    ev_stale.result.char_id = "stale_char".to_string();
    app.world_mut()
        .resource_mut::<Events<TribulationSettled>>()
        .send(ev_stale);
    app.update();
    // 推进到过窗
    let past_window = 100 + RECHALLENGE_WINDOW_TICKS + 1;
    app.world_mut().resource_mut::<CombatClock>().tick = past_window;
    // 第二个 HalfStep entity（在过窗时刻入队），独立 char_id 保证拿新窗口
    let fresh = app.world_mut().spawn(()).id();
    let mut ev_fresh = make_settled_event_with_source(
        fresh,
        DuXuOutcomeV1::HalfStep,
        JueBiTriggerSource::VoidQuotaExceeded,
    );
    ev_fresh.result.char_id = "fresh_char".to_string();
    app.world_mut()
        .resource_mut::<Events<TribulationSettled>>()
        .send(ev_fresh);
    app.update();
    // 派发：stale 应过窗 drop，fresh 应收到
    app.world_mut()
        .resource_mut::<Events<AscensionQuotaOpened>>()
        .send(AscensionQuotaOpened { occupied_slots: 1 });
    app.update();
    let triggers = drain_rechallenge_triggers(&mut app);
    assert_eq!(triggers.len(), 1);
    assert_eq!(
        triggers[0].entity, fresh,
        "stale entry 应被丢弃，fresh entry 收到通知（§8 Q1 7d 窗口）"
    );
    assert!(app
        .world()
        .resource::<HalfStepRechallengeQueue>()
        .is_empty());
}
#[test]
fn dispatch_no_op_on_empty_queue() {
    let mut app = p0_metrics_test_app();
    app.world_mut()
        .resource_mut::<Events<AscensionQuotaOpened>>()
        .send(AscensionQuotaOpened { occupied_slots: 0 });
    app.update();
    let triggers = drain_rechallenge_triggers(&mut app);
    assert!(
        triggers.is_empty(),
        "空队列应无声 no-op；触发空事件违反幂等性"
    );
}
#[test]
fn dispatch_drains_multiple_events_in_fifo_order() {
    let mut app = p0_metrics_test_app();
    app.world_mut().resource_mut::<CombatClock>().tick = 100;
    // 三个 HalfStep entity，按时间序入队。
    // 注意每个 char_id 必须独立——否则 char_id dedup（P3 review #3 修复引入）
    // 会把前两条清掉只剩最后一条
    let mut entities = Vec::new();
    let char_ids = ["char_alpha", "char_bravo", "char_charlie"];
    for (i, et) in [100u64, 200, 300].iter().enumerate() {
        app.world_mut().resource_mut::<CombatClock>().tick = *et;
        let e = app.world_mut().spawn(()).id();
        entities.push(e);
        let mut ev = make_settled_event_with_source(
            e,
            DuXuOutcomeV1::HalfStep,
            JueBiTriggerSource::VoidQuotaExceeded,
        );
        ev.result.char_id = char_ids[i].to_string();
        app.world_mut()
            .resource_mut::<Events<TribulationSettled>>()
            .send(ev);
        app.update();
    }
    assert_eq!(app.world().resource::<HalfStepRechallengeQueue>().len(), 3);
    // 一次发送 3 个 AscensionQuotaOpened events
    for _ in 0..3 {
        app.world_mut()
            .resource_mut::<Events<AscensionQuotaOpened>>()
            .send(AscensionQuotaOpened { occupied_slots: 0 });
    }
    app.update();
    let triggers = drain_rechallenge_triggers(&mut app);
    assert_eq!(
        triggers.len(),
        3,
        "3 个 AscensionQuotaOpened 应耗尽队列发 3 个 trigger"
    );
    // FIFO：早入队的先 trigger
    assert_eq!(triggers[0].entity, entities[0]);
    assert_eq!(triggers[1].entity, entities[1]);
    assert_eq!(triggers[2].entity, entities[2]);
    assert!(app
        .world()
        .resource::<HalfStepRechallengeQueue>()
        .is_empty());
}
#[test]
fn tribulation_aoe_wave_drain_credits_zone() {
    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 1200 });
    app.add_event::<TribulationFailed>();
    app.add_event::<DeathEvent>();
    app.add_event::<QiTransfer>();

    // Empty zone so there is room for 35 qi (cap = QI_ZONE_UNIT_CAPACITY = 50).
    let mut zones = ZoneRegistry::fallback();
    zones
        .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
        .expect("fallback zone should exist")
        .spirit_qi = 0.0;
    app.insert_resource(zones);
    app.add_systems(Update, tribulation_aoe_system);

    // DuXu wave 1: epicenter at origin, this tick = phase_started_tick
    app.world_mut().spawn(TribulationState {
        kind: TribulationKind::DuXu,
        phase: TribulationPhase::Wave(1),
        epicenter: [0.0, 64.0, 0.0],
        wave_current: 1,
        waves_total: 3,
        started_tick: 0,
        phase_started_tick: 1200,
        next_wave_tick: 1500,
        participants: vec!["offline:Azure".to_string()],
        failed: false,
    });

    // Target at distance 0, within TRIBULATION_DANGER_RADIUS. Must have
    // CurrentDimension so find_zone can resolve the spawn zone.
    let target = app
        .world_mut()
        .spawn((
            Position::new([0.0, 64.0, 0.0]),
            CurrentDimension(DimensionKind::Overworld),
            Cultivation {
                realm: Realm::Spirit,
                qi_current: 200.0,
                qi_max: 210.0,
                ..Default::default()
            },
            Wounds {
                health_current: 200.0,
                health_max: 200.0,
                entries: Vec::new(),
            },
            Lifecycle {
                character_id: "offline:Azure".to_string(),
                ..Default::default()
            },
            LifeRecord::new("offline:Azure"),
        ))
        .id();

    app.update();

    let cultivation = app
        .world()
        .get::<Cultivation>(target)
        .expect("cultivation should remain");
    let expected_after = 200.0 - DUXU_QI_DRAIN_BASE; // wave 1 × 35
    assert_eq!(
        cultivation.qi_current, expected_after,
        "qi_current should be reduced by wave-1 drain ({DUXU_QI_DRAIN_BASE})"
    );

    // Zone spirit_qi should have risen by exactly drain/QI_ZONE_UNIT_CAPACITY = 35/50 = 0.70
    let zone_after = app
        .world()
        .resource::<ZoneRegistry>()
        .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
        .expect("fallback zone should exist")
        .spirit_qi;
    let expected_zone = DUXU_QI_DRAIN_BASE / 50.0; // 0.70
    assert!(
        (zone_after - expected_zone).abs() < 1e-9,
        "zone spirit_qi should rise by drain/cap: expected {expected_zone}, got {zone_after}"
    );

    // At least one QiTransfer event must have been emitted for the zone credit.
    let transfers: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<QiTransfer>>()
        .drain()
        .collect();
    assert!(
        !transfers.is_empty(),
        "tribulation_aoe wave drain must emit a QiTransfer event; got none"
    );
    assert!(
        transfers
            .iter()
            .any(|t| t.reason == QiTransferReason::ReleaseToZone),
        "at least one transfer should have reason ReleaseToZone"
    );
}
#[test]
fn tribulation_aoe_wave_drain_zero_qi_no_credit() {
    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 1200 });
    app.add_event::<TribulationFailed>();
    app.add_event::<DeathEvent>();
    app.add_event::<QiTransfer>();

    let mut zones = ZoneRegistry::fallback();
    zones
        .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
        .expect("fallback zone should exist")
        .spirit_qi = 0.0;
    app.insert_resource(zones);
    app.add_systems(Update, tribulation_aoe_system);

    app.world_mut().spawn(TribulationState {
        kind: TribulationKind::DuXu,
        phase: TribulationPhase::Wave(1),
        epicenter: [0.0, 64.0, 0.0],
        wave_current: 1,
        waves_total: 3,
        started_tick: 0,
        phase_started_tick: 1200,
        next_wave_tick: 1500,
        participants: vec!["offline:Azure".to_string()],
        failed: false,
    });

    app.world_mut().spawn((
        Position::new([0.0, 64.0, 0.0]),
        CurrentDimension(DimensionKind::Overworld),
        Cultivation {
            realm: Realm::Spirit,
            qi_current: 0.0,
            qi_max: 210.0,
            ..Default::default()
        },
        Wounds {
            health_current: 200.0,
            health_max: 200.0,
            entries: Vec::new(),
        },
        Lifecycle {
            character_id: "offline:Azure".to_string(),
            ..Default::default()
        },
    ));

    app.update();

    let zone_after = app
        .world()
        .resource::<ZoneRegistry>()
        .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
        .expect("fallback zone should exist")
        .spirit_qi;
    assert_eq!(
        zone_after, 0.0,
        "zero-qi player produces zero actual drain; zone must stay at 0.0"
    );
    let transfers: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<QiTransfer>>()
        .drain()
        .collect();
    assert!(
        transfers.is_empty(),
        "zero actual drain must not emit any QiTransfer; got {}",
        transfers.len()
    );
}
#[test]
fn juebi_pressure_collapse_wave1_drain_credits_zone() {
    let mut app = qi_test_app();
    // tick != phase_started_tick so per-tick phase damage is not triggered
    app.insert_resource(CombatClock { tick: 101 });
    app.insert_resource(JueBiNullFields::default());
    app.add_event::<DeathEvent>();
    app.add_event::<QiTransfer>();

    let mut zones = ZoneRegistry::fallback();
    zones
        .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
        .expect("fallback zone should exist")
        .spirit_qi = 0.0;
    app.insert_resource(zones);
    app.add_systems(Update, juebi_phase_effect_system);

    app.world_mut().spawn(TribulationState {
        kind: TribulationKind::JueBi,
        phase: TribulationPhase::Wave(1),
        epicenter: [0.0, 64.0, 0.0],
        wave_current: 1,
        waves_total: JUEBI_WAVES_TOTAL,
        started_tick: 0,
        phase_started_tick: 100,
        next_wave_tick: 100 + JUEBI_PHASE_TICKS,
        participants: vec!["offline:Azure".to_string()],
        failed: false,
    });

    // Target at epicenter (distance 0 → factor = 1.0, max drain rate).
    // Default intensity (JueBiRuntimeContext absent) = JUEBI_INTENSITY_BASE → scale = 1.0.
    // CurrentDimension required so zone lookup succeeds.
    let target = app
        .world_mut()
        .spawn((
            Position::new([0.0, 64.0, 0.0]),
            CurrentDimension(DimensionKind::Overworld),
            Cultivation {
                realm: Realm::Void,
                qi_current: 100.0,
                qi_max: 100.0,
                ..Default::default()
            },
            Wounds::default(),
            Lifecycle {
                character_id: "offline:Azure".to_string(),
                ..Default::default()
            },
            LifeRecord::new("offline:Azure"),
        ))
        .id();

    app.update();

    let cultivation = app
        .world()
        .get::<Cultivation>(target)
        .expect("cultivation should remain");
    let expected_drain = 100.0 * JUEBI_PRESSURE_DRAIN_PER_TICK; // 2.0
    let expected_after = 100.0 - expected_drain;
    assert!(
        (cultivation.qi_current - expected_after).abs() < 1e-9,
        "qi_current after wave-1 pressure drain: expected {expected_after}, got {}",
        cultivation.qi_current
    );

    let zone_after = app
        .world()
        .resource::<ZoneRegistry>()
        .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
        .expect("fallback zone should exist")
        .spirit_qi;
    // expected zone credit = expected_drain / QI_ZONE_UNIT_CAPACITY = 2.0 / 50.0 = 0.04
    let expected_zone = expected_drain / 50.0;
    assert!(
        (zone_after - expected_zone).abs() < 1e-9,
        "zone spirit_qi after juebi wave-1 pressure drain: expected {expected_zone}, got {zone_after}"
    );

    let transfers: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<QiTransfer>>()
        .drain()
        .collect();
    assert!(
        !transfers.is_empty(),
        "juebi wave-1 pressure drain must emit a QiTransfer event"
    );
}
#[test]
fn juebi_null_field_wave3_drain_credits_zone() {
    let mut app = qi_test_app();
    // tick = phase_started_tick + 100 (elapsed=100 → null_radius = 50.0)
    app.insert_resource(CombatClock { tick: 100 });
    app.insert_resource(JueBiNullFields::default());
    app.add_event::<DeathEvent>();
    app.add_event::<QiTransfer>();

    let mut zones = ZoneRegistry::fallback();
    zones
        .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
        .expect("fallback zone should exist")
        .spirit_qi = 0.0;
    app.insert_resource(zones);
    app.add_systems(Update, juebi_phase_effect_system);

    // phase_started_tick = 0; clock.tick = 100; elapsed = 100
    // null_radius = 150.0 × (100 / 300) = 50.0
    app.world_mut().spawn(TribulationState {
        kind: TribulationKind::JueBi,
        phase: TribulationPhase::Wave(3),
        epicenter: [0.0, 64.0, 0.0],
        wave_current: 3,
        waves_total: JUEBI_WAVES_TOTAL,
        started_tick: 0,
        phase_started_tick: 0,
        next_wave_tick: JUEBI_PHASE_TICKS,
        participants: vec!["offline:Azure".to_string()],
        failed: false,
    });

    // Void realm so juebi_null_decay_for_realm returns JUEBI_NULL_VOID_DECAY_PER_TICK (0.03).
    // Position at distance 0 (< null_radius 50.0) → inside the expanding null field.
    let target = app
        .world_mut()
        .spawn((
            Position::new([0.0, 64.0, 0.0]),
            CurrentDimension(DimensionKind::Overworld),
            Cultivation {
                realm: Realm::Void,
                qi_current: 100.0,
                qi_max: 100.0,
                ..Default::default()
            },
            Wounds::default(),
            Lifecycle {
                character_id: "offline:Azure".to_string(),
                ..Default::default()
            },
            LifeRecord::new("offline:Azure"),
        ))
        .id();

    app.update();

    let cultivation = app
        .world()
        .get::<Cultivation>(target)
        .expect("cultivation should remain");
    // decay = 0.03 × 1.0 (intensity_scale) = 0.03; after = 100 × (1 - 0.03) = 97.0
    let expected_after = 100.0 * (1.0 - JUEBI_NULL_VOID_DECAY_PER_TICK);
    let actual_drain = 100.0 - expected_after; // 3.0
    assert!(
        (cultivation.qi_current - expected_after).abs() < 1e-9,
        "qi_current after wave-3 null field decay: expected {expected_after}, got {}",
        cultivation.qi_current
    );

    let zone_after = app
        .world()
        .resource::<ZoneRegistry>()
        .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
        .expect("fallback zone should exist")
        .spirit_qi;
    let expected_zone = actual_drain / 50.0; // 3.0 / 50.0 = 0.06
    assert!(
        (zone_after - expected_zone).abs() < 1e-9,
        "zone spirit_qi after juebi wave-3 null field drain: expected {expected_zone}, got {zone_after}"
    );

    let transfers: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<QiTransfer>>()
        .drain()
        .collect();
    assert!(
        !transfers.is_empty(),
        "juebi wave-3 null field drain must emit a QiTransfer event"
    );
}
#[test]
fn juebi_null_field_wave3_no_drain_outside_radius() {
    let mut app = qi_test_app();
    // elapsed = 1 tick → null_radius = 150.0 × (1/300) ≈ 0.5 (essentially 0)
    app.insert_resource(CombatClock { tick: 1 });
    app.insert_resource(JueBiNullFields::default());
    app.add_event::<DeathEvent>();
    app.add_event::<QiTransfer>();

    let mut zones = ZoneRegistry::fallback();
    zones
        .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
        .expect("fallback zone should exist")
        .spirit_qi = 0.0;
    app.insert_resource(zones);
    app.add_systems(Update, juebi_phase_effect_system);

    // phase_started_tick = 0; clock.tick = 1; elapsed = 1 → null_radius ≈ 0.5
    app.world_mut().spawn(TribulationState {
        kind: TribulationKind::JueBi,
        phase: TribulationPhase::Wave(3),
        epicenter: [0.0, 64.0, 0.0],
        wave_current: 3,
        waves_total: JUEBI_WAVES_TOTAL,
        started_tick: 0,
        phase_started_tick: 0,
        next_wave_tick: JUEBI_PHASE_TICKS,
        participants: vec!["offline:Azure".to_string()],
        failed: false,
    });

    // Target at distance 10.0, outside null_radius ≈ 0.5 → skipped by wave-3 branch.
    app.world_mut().spawn((
        Position::new([10.0, 64.0, 0.0]),
        CurrentDimension(DimensionKind::Overworld),
        Cultivation {
            realm: Realm::Void,
            qi_current: 100.0,
            qi_max: 100.0,
            ..Default::default()
        },
        Wounds::default(),
        Lifecycle {
            character_id: "offline:Azure".to_string(),
            ..Default::default()
        },
    ));

    app.update();

    let zone_after = app
        .world()
        .resource::<ZoneRegistry>()
        .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
        .expect("fallback zone should exist")
        .spirit_qi;
    assert_eq!(
        zone_after, 0.0,
        "target outside null radius must not generate zone credit"
    );
    let transfers: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<QiTransfer>>()
        .drain()
        .collect();
    assert!(
        transfers.is_empty(),
        "target outside null radius must not emit any QiTransfer"
    );
}
