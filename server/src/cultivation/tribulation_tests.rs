use super::*;
use crate::combat::components::{Lifecycle, Wounds};
use crate::combat::events::DeathEvent;
use crate::combat::CombatClock;
use crate::cultivation::components::MeridianId;
use crate::cultivation::life_record::{BiographyEntry, LifeRecord};
use crate::inventory::{
    ContainerState, InventoryRevision, ItemInstance, ItemRarity, PlacedItemState, PlayerInventory,
    MAIN_PACK_CONTAINER_ID,
};
use crate::network::vfx_event_emit::VfxEventRequest;
use crate::persistence::bootstrap_sqlite;
use crate::qi_physics::QiTransfer;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use valence::prelude::{App, Entity, Events, Position, Update};
use valence::testing::ScenarioSingleClient;

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
fn spawn_tribulation_spectator(app: &mut App, name: &str, pos: [f64; 3]) -> Entity {
    let character_id = format!("offline:{name}");
    app.world_mut()
        .spawn((
            Position::new(pos),
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
                character_id: character_id.clone(),
                ..Default::default()
            },
            LifeRecord::new(character_id),
        ))
        .id()
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
fn full_progress_life_record(spirit_tick: u64, final_meridian_tick: u64) -> LifeRecord {
    let mut record = LifeRecord::new("offline:Azure");
    record.push(BiographyEntry::BreakthroughSucceeded {
        realm: Realm::Spirit,
        tick: spirit_tick,
    });
    let meridians: Vec<_> = MeridianId::REGULAR
        .iter()
        .chain(MeridianId::EXTRAORDINARY.iter())
        .copied()
        .collect();
    let count = meridians.len().saturating_sub(1) as u64;
    for (idx, id) in meridians.into_iter().enumerate() {
        record.push(BiographyEntry::MeridianOpened {
            id,
            tick: final_meridian_tick.saturating_sub(count.saturating_sub(idx as u64)),
        });
    }
    record
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
fn tribulation_omen_cloud_blocks_overlay_and_restore() {
    let scenario = ScenarioSingleClient::new();
    let layer = scenario.layer;
    let mut app = scenario.app;
    crate::world::dimension::mark_test_layer_as_overworld(&mut app);
    app.world_mut()
        .get_mut::<ChunkLayer>(layer)
        .expect("test layer should carry ChunkLayer")
        .insert_chunk([0, 0], valence::prelude::UnloadedChunk::new());
    app.insert_resource(CombatClock { tick: 0 });
    app.insert_resource(TribulationOmenCloudBlocks::default());
    app.add_event::<TribulationAnnounce>();
    app.add_systems(Update, tribulation_omen_cloud_block_overlay_system);

    let entity = app
        .world_mut()
        .spawn(TribulationState {
            kind: TribulationKind::DuXu,
            phase: TribulationPhase::Omen,
            epicenter: [8.0, 66.0, 8.0],
            wave_current: 0,
            waves_total: 3,
            started_tick: 0,
            phase_started_tick: 0,
            next_wave_tick: DUXU_OMEN_TICKS + DUXU_LOCK_TICKS,
            participants: vec!["offline:Azure".to_string()],
            failed: false,
        })
        .id();
    app.world_mut().send_event(TribulationAnnounce {
        entity,
        char_id: "offline:Azure".to_string(),
        actor_name: "Azure".to_string(),
        epicenter: [8.0, 66.0, 8.0],
        waves_total: 3,
        started_tick: 0,
    });

    app.update();

    let center = BlockPos::new(8, 90, 8);
    let edge = BlockPos::new(12, 90, 8);
    {
        let layer_ref = app
            .world()
            .get::<ChunkLayer>(layer)
            .expect("test layer should carry ChunkLayer");
        assert_eq!(
            layer_ref.block(center).map(|block| block.state),
            Some(BlockState::BLACK_WOOL)
        );
        assert_eq!(
            layer_ref.block(edge).map(|block| block.state),
            Some(BlockState::WHITE_WOOL)
        );
    }

    app.world_mut()
        .entity_mut(entity)
        .remove::<TribulationState>();
    app.world_mut().resource_mut::<CombatClock>().tick = DUXU_OMEN_TICKS;
    app.update();

    let layer_ref = app.world().get::<ChunkLayer>(layer).unwrap();
    assert_eq!(
        layer_ref.block(center).map(|block| block.state),
        Some(BlockState::AIR)
    );
    assert_eq!(
        layer_ref.block(edge).map(|block| block.state),
        Some(BlockState::AIR)
    );
}

#[test]
fn long_full_progress_du_xu_request_adds_heart_demon_and_kaitian_waves() {
    let mut app = qi_test_app();
    app.add_event::<StartDuXuRequest>();
    app.add_event::<InitiateXuhuaTribulation>();
    app.add_systems(Update, start_du_xu_request_system);
    let requested_at_tick = DUXU_FULL_PROGRESS_MIN_TICKS + 500;
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
            full_progress_life_record(100, 500),
        ))
        .id();

    app.world_mut().send_event(StartDuXuRequest {
        entity,
        requested_at_tick,
    });
    app.update();

    let events = app.world().resource::<Events<InitiateXuhuaTribulation>>();
    let emitted: Vec<_> = events.get_reader().read(events).cloned().collect();
    assert_eq!(emitted.len(), 1);
    assert_eq!(emitted[0].entity, entity);
    assert_eq!(emitted[0].waves_total, 5);
    assert_eq!(emitted[0].started_tick, requested_at_tick);
}
#[test]
fn recent_full_progress_du_xu_request_keeps_default_three_waves() {
    let mut app = qi_test_app();
    app.add_event::<StartDuXuRequest>();
    app.add_event::<InitiateXuhuaTribulation>();
    app.add_systems(Update, start_du_xu_request_system);
    let requested_at_tick = DUXU_FULL_PROGRESS_MIN_TICKS + 500;
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
            full_progress_life_record(100, requested_at_tick - 1),
        ))
        .id();

    app.world_mut().send_event(StartDuXuRequest {
        entity,
        requested_at_tick,
    });
    app.update();

    let events = app.world().resource::<Events<InitiateXuhuaTribulation>>();
    let emitted: Vec<_> = events.get_reader().read(events).cloned().collect();
    assert_eq!(emitted.len(), 1);
    assert_eq!(emitted[0].waves_total, 3);
}
#[test]
fn full_progress_boundary_35999_keeps_three_waves() {
    // review finding major-6：差一 tick 侧必须用**字面 35999** 锁死（不能用符号常量
    // 配"差一历史"的旧写法——把门槛改成 35999 或谓词改成 > MIN 会让旧测试全绿）。
    let spirit_tick = 100u64;
    let final_meridian_tick = 500u64;
    let record = full_progress_life_record(spirit_tick, final_meridian_tick);
    // du_xu_full_progress_ticks = requested - max(spirit, final_meridian) = requested - 500
    let requested_at_tick = DUXU_FULL_PROGRESS_MIN_TICKS + final_meridian_tick - 1; // = 36499
    assert_eq!(
        du_xu_full_progress_ticks(&record, requested_at_tick),
        35_999,
        "满进度 ticks 必须恰为 35999（差一 tick 不到 36000 门槛）"
    );
    assert_eq!(
        du_xu_waves_total(requested_at_tick, Some(&record)),
        DUXU_DEFAULT_WAVES,
        "35999 ticks 未过门槛，必须是默认 3 波"
    );

    // 经真实 system 走同一字面输入，锁系流程级语义。
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
            full_progress_life_record(spirit_tick, final_meridian_tick),
        ))
        .id();
    app.world_mut().send_event(StartDuXuRequest {
        entity,
        requested_at_tick,
    });
    app.update();
    let events = app.world().resource::<Events<InitiateXuhuaTribulation>>();
    let emitted: Vec<_> = events.get_reader().read(events).cloned().collect();
    assert_eq!(emitted.len(), 1);
    assert_eq!(emitted[0].waves_total, 3);
}
#[test]
fn full_progress_boundary_36000_adds_five_waves() {
    // review finding major-6：门限侧用**字面 36000** 锁死，谓词/常量若被放宽成
    // <= 35999 或 > 35999，本字面断言立刻撞红。
    let spirit_tick = 100u64;
    let final_meridian_tick = 500u64;
    let record = full_progress_life_record(spirit_tick, final_meridian_tick);
    let requested_at_tick = DUXU_FULL_PROGRESS_MIN_TICKS + final_meridian_tick; // = 36500
    assert_eq!(
        du_xu_full_progress_ticks(&record, requested_at_tick),
        36_000,
        "满进度 ticks 必须恰为 36000（恰好触及 36000 门槛）"
    );
    assert_eq!(
        du_xu_waves_total(requested_at_tick, Some(&record)),
        DUXU_MAX_WAVES,
        "36000 ticks 恰达门槛，必须是 5 波（含心魔相）"
    );

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
            full_progress_life_record(spirit_tick, final_meridian_tick),
        ))
        .id();
    app.world_mut().send_event(StartDuXuRequest {
        entity,
        requested_at_tick,
    });
    app.update();
    let events = app.world().resource::<Events<InitiateXuhuaTribulation>>();
    let emitted: Vec<_> = events.get_reader().read(events).cloned().collect();
    assert_eq!(emitted.len(), 1);
    assert_eq!(emitted[0].waves_total, 5);
}
#[test]
fn resolved_heart_demon_after_soul_devouring_skips_original_heart_demon_slot() {
    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 2110 });
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
                wave_current: 3,
                waves_total: 5,
                started_tick: 100,
                phase_started_tick: 1800,
                next_wave_tick: 2100,
                participants: vec!["offline:Azure".to_string()],
                failed: false,
            },
            HeartDemonResolution {
                outcome: HeartDemonOutcome::Steadfast,
                choice_idx: Some(0),
                tick: 1810,
                next_wave_multiplier: 1.0,
            },
        ))
        .id();

    app.update();

    let state = app
        .world()
        .get::<TribulationState>(entity)
        .expect("tribulation should remain active");
    assert_eq!(state.phase, TribulationPhase::Wave(DUXU_KAITIAN_WAVE));
    assert_eq!(state.wave_current, 3);
    let events = app.world().resource::<Events<TribulationWaveCleared>>();
    let emitted: Vec<_> = events.get_reader().read(events).cloned().collect();
    assert_eq!(emitted.len(), 1);
    assert_eq!(emitted[0].wave, DUXU_KAITIAN_WAVE);
}
#[test]
fn heart_demon_obsession_timeout_penalizes_qi_and_boosts_kaitian_damage() {
    let mut app = qi_test_app();
    app.insert_resource(CombatClock {
        tick: 2100 + DUXU_HEART_DEMON_TIMEOUT_TICKS,
    });
    app.add_event::<TribulationWaveCleared>();
    app.add_systems(Update, heart_demon_timeout_system);
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

    app.update();

    let cultivation = app
        .world()
        .get::<Cultivation>(entity)
        .expect("cultivation should remain attached");
    assert_eq!(cultivation.qi_current, 70.0);
    let resolution = app
        .world()
        .get::<HeartDemonResolution>(entity)
        .expect("resolution should be recorded");
    assert_eq!(resolution.outcome, HeartDemonOutcome::Obsession);
    assert_eq!(resolution.choice_idx, None);
    assert_eq!(
        resolution.next_wave_multiplier,
        DUXU_HEART_DEMON_OBSESSION_NEXT_WAVE_MULTIPLIER
    );
}
#[test]
fn heart_demon_resolution_advances_to_kaitian_without_republishing_fourth_wave() {
    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 2140 });
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
                wave_current: 4,
                waves_total: 5,
                started_tick: 0,
                phase_started_tick: 2100,
                next_wave_tick: 2400,
                participants: vec!["offline:Azure".to_string()],
                failed: false,
            },
            HeartDemonResolution {
                outcome: HeartDemonOutcome::Obsession,
                choice_idx: None,
                tick: 2130,
                next_wave_multiplier: DUXU_HEART_DEMON_OBSESSION_NEXT_WAVE_MULTIPLIER,
            },
        ))
        .id();

    app.update();

    let state = app
        .world()
        .get::<TribulationState>(entity)
        .expect("tribulation should remain active");
    assert_eq!(state.phase, TribulationPhase::Wave(5));
    assert_eq!(state.phase_started_tick, 2140);
    let events = app.world().resource::<Events<TribulationWaveCleared>>();
    let emitted: Vec<_> = events.get_reader().read(events).cloned().collect();
    assert_eq!(emitted.len(), 1);
    assert_eq!(emitted[0].wave, 5);
}
#[test]
fn obsession_resolution_increases_kaitian_damage() {
    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 2400 });
    app.add_event::<TribulationFailed>();
    app.add_event::<DeathEvent>();
    app.add_systems(Update, tribulation_aoe_system);
    let entity = app
        .world_mut()
        .spawn((
            Position::new([0.0, 66.0, 0.0]),
            Cultivation {
                realm: Realm::Spirit,
                qi_current: 300.0,
                qi_max: 300.0,
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
                phase_started_tick: 2400,
                next_wave_tick: 2700,
                participants: vec!["offline:Azure".to_string()],
                failed: false,
            },
            HeartDemonResolution {
                outcome: HeartDemonOutcome::Obsession,
                choice_idx: None,
                tick: 2130,
                next_wave_multiplier: DUXU_HEART_DEMON_OBSESSION_NEXT_WAVE_MULTIPLIER,
            },
        ))
        .id();

    app.update();

    assert_eq!(app.world().resource::<Events<TribulationFailed>>().len(), 0);
    let wounds = app
        .world()
        .get::<Wounds>(entity)
        .expect("wounds should remain attached");
    assert_eq!(
        wounds.health_current,
        200.0 - DUXU_AOE_DAMAGE_BASE * 5.0 * DUXU_HEART_DEMON_OBSESSION_NEXT_WAVE_MULTIPLIER
    );
    assert_eq!(wounds.entries.len(), 1);
    assert_eq!(
        wounds.entries[0].severity,
        DUXU_AOE_DAMAGE_BASE * 5.0 * DUXU_HEART_DEMON_OBSESSION_NEXT_WAVE_MULTIPLIER
    );
}
#[test]
fn phase7_balance_three_wave_curve_fits_spirit_pool() {
    let spirit_pool = 210.0;
    let profiles = (1..=3).map(du_xu_wave_profile).collect::<Vec<_>>();
    assert_eq!(profiles[0].damage, 18.0);
    assert_eq!(profiles[1].damage, 36.0);
    assert_eq!(profiles[2].damage, 54.0);
    assert_eq!(profiles[0].qi_drain, 35.0);
    assert_eq!(profiles[1].qi_drain, 70.0);
    assert_eq!(profiles[2].qi_drain, 105.0);
    assert_eq!(
        profiles.iter().map(|profile| profile.damage).sum::<f32>(),
        108.0
    );
    assert_eq!(
        profiles.iter().map(|profile| profile.qi_drain).sum::<f64>(),
        210.0
    );
    assert!(profiles.iter().map(|profile| profile.qi_drain).sum::<f64>() <= spirit_pool);
}
#[test]
fn aoe_uses_current_wave_strength_only_on_wave_start_tick() {
    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 1200 });
    app.add_event::<TribulationFailed>();
    app.add_event::<DeathEvent>();
    app.add_systems(Update, tribulation_aoe_system);

    app.world_mut().spawn(TribulationState {
        kind: TribulationKind::DuXu,
        phase: TribulationPhase::Wave(2),
        epicenter: [0.0, 66.0, 0.0],
        wave_current: 1,
        waves_total: 3,
        started_tick: 0,
        phase_started_tick: 1200,
        next_wave_tick: 1500,
        participants: vec!["offline:Azure".to_string()],
        failed: false,
    });
    let target = app
        .world_mut()
        .spawn((
            Position::new([8.0, 66.0, 0.0]),
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
    assert_eq!(wounds.health_current, 100.0 - DUXU_AOE_DAMAGE_BASE * 2.0);
    assert_eq!(wounds.entries.len(), DUXU_CHAIN_LIGHTNING_STRIKES as usize);
    for wound in &wounds.entries {
        assert_eq!(wound.kind, WoundKind::Burn);
        assert_eq!(wound.severity, DUXU_AOE_DAMAGE_BASE * 2.0 / 3.0);
        assert_eq!(wound.created_at_tick, 1200);
        assert_eq!(wound.inflicted_by.as_deref(), Some("du_xu_tribulation"));
    }
    let cultivation = app
        .world()
        .get::<Cultivation>(target)
        .expect("cultivation should remain attached");
    assert_eq!(cultivation.qi_current, 100.0 - DUXU_QI_DRAIN_BASE * 2.0);
    assert_eq!(cultivation.qi_max_frozen, None);

    app.world_mut().resource_mut::<CombatClock>().tick = 1201;
    app.update();

    let wounds = app
        .world()
        .get::<Wounds>(target)
        .expect("wounds should remain attached");
    assert_eq!(wounds.health_current, 100.0 - DUXU_AOE_DAMAGE_BASE * 2.0);
    assert_eq!(wounds.entries.len(), DUXU_CHAIN_LIGHTNING_STRIKES as usize);
}
#[test]
fn spectator_aoe_is_not_reduced_by_distance_within_danger_radius() {
    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 1200 });
    app.add_event::<TribulationFailed>();
    app.add_event::<DeathEvent>();
    app.add_systems(Update, tribulation_aoe_system);

    app.world_mut().spawn(TribulationState {
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
    });
    let near = spawn_tribulation_spectator(&mut app, "Near", [3.0, 66.0, 0.0]);
    let far_inside = spawn_tribulation_spectator(
        &mut app,
        "FarInside",
        [TRIBULATION_DANGER_RADIUS, 66.0, 0.0],
    );
    let outside = spawn_tribulation_spectator(
        &mut app,
        "Outside",
        [TRIBULATION_DANGER_RADIUS + 0.1, 66.0, 0.0],
    );

    app.update();

    let expected_health = 200.0 - DUXU_AOE_DAMAGE_BASE * 2.0;
    let expected_qi = 200.0 - DUXU_QI_DRAIN_BASE * 2.0;
    for entity in [near, far_inside] {
        let wounds = app
            .world()
            .get::<Wounds>(entity)
            .expect("spectator wounds should remain attached");
        assert_eq!(wounds.health_current, expected_health);
        assert_eq!(wounds.entries.len(), DUXU_CHAIN_LIGHTNING_STRIKES as usize);
        let cultivation = app
            .world()
            .get::<Cultivation>(entity)
            .expect("spectator cultivation should remain attached");
        assert_eq!(cultivation.qi_current, expected_qi);
    }
    let outside_wounds = app
        .world()
        .get::<Wounds>(outside)
        .expect("outside spectator wounds should remain attached");
    assert_eq!(outside_wounds.health_current, 200.0);
    assert!(outside_wounds.entries.is_empty());
    let outside_cultivation = app
        .world()
        .get::<Cultivation>(outside)
        .expect("outside spectator cultivation should remain attached");
    assert_eq!(outside_cultivation.qi_current, 200.0);
}
#[test]
fn third_wave_freezes_qi_max_as_soul_devouring_lightning() {
    let mut app = qi_test_app();
    app.insert_resource(CombatClock { tick: 1500 });
    app.add_event::<TribulationFailed>();
    app.add_event::<DeathEvent>();
    app.add_systems(Update, tribulation_aoe_system);

    app.world_mut().spawn(TribulationState {
        kind: TribulationKind::DuXu,
        phase: TribulationPhase::Wave(3),
        epicenter: [0.0, 66.0, 0.0],
        wave_current: 3,
        waves_total: 3,
        started_tick: 0,
        phase_started_tick: 1500,
        next_wave_tick: 1800,
        participants: vec!["offline:Azure".to_string()],
        failed: false,
    });
    let target = app
        .world_mut()
        .spawn((
            Position::new([8.0, 66.0, 0.0]),
            Cultivation {
                realm: Realm::Spirit,
                qi_current: 200.0,
                qi_max: 210.0,
                qi_max_frozen: Some(10.0),
                ..Default::default()
            },
            Wounds {
                health_current: 200.0,
                health_max: 200.0,
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
    assert_eq!(wounds.health_current, 200.0 - DUXU_AOE_DAMAGE_BASE * 3.0);
    assert_eq!(wounds.entries.len(), 1);
    assert_eq!(wounds.entries[0].severity, DUXU_AOE_DAMAGE_BASE * 3.0);
    let cultivation = app
        .world()
        .get::<Cultivation>(target)
        .expect("cultivation should remain attached");
    assert_eq!(cultivation.qi_current, 200.0 - DUXU_QI_DRAIN_BASE * 3.0);
    let expected_frozen = 10.0 + 210.0 * DUXU_SOUL_DEVOUR_QI_MAX_FREEZE_RATIO;
    assert!(
        (cultivation.qi_max_frozen.expect("qi max should freeze") - expected_frozen).abs()
            < f64::EPSILON
    );
}
#[test]
fn juebi_terrain_generation_keeps_animation_order() {
    let mut pending = VecDeque::new();
    enqueue_juebi_terrain_ops(&mut pending, [0.0, 64.0, 0.0], 42, 6_000);
    assert!(pending.len() > 10_000);
    let mut last = 0;
    for op in pending {
        assert!(op.anim_order >= last);
        assert_eq!(op.restore_at_tick, 6_000);
        last = op.anim_order;
    }
}
#[test]
fn juebi_terrain_tick_skips_unloaded_chunks_without_air_restore() {
    let scenario = ScenarioSingleClient::new();
    let mut app = scenario.app;
    crate::world::dimension::mark_test_layer_as_overworld(&mut app);
    app.insert_resource(CombatClock { tick: 10 });
    let mut overlay = JueBiTerrainOverlay::default();
    overlay.pending.push_back(TerrainModOp {
        pos: BlockPos::new(32, 64, 32),
        new_state: BlockState::MAGMA_BLOCK,
        anim_order: 0,
        restore_at_tick: 6_000,
    });
    app.insert_resource(overlay);
    app.add_systems(Update, juebi_terrain_tick_system);

    app.update();

    let overlay = app.world().resource::<JueBiTerrainOverlay>();
    assert!(
        overlay.placed.is_empty(),
        "unloaded chunks must not record AIR as original block for later restore"
    );
    assert!(overlay.pending.is_empty());
}
#[test]
fn juebi_terrain_tick_records_original_once_for_overlapping_ops() {
    let scenario = ScenarioSingleClient::new();
    let layer = scenario.layer;
    let mut app = scenario.app;
    crate::world::dimension::mark_test_layer_as_overworld(&mut app);
    app.world_mut()
        .get_mut::<ChunkLayer>(layer)
        .expect("test layer should carry ChunkLayer")
        .insert_chunk([0, 0], valence::prelude::UnloadedChunk::new());
    let pos = BlockPos::new(1, 64, 1);
    app.world_mut()
        .get_mut::<ChunkLayer>(layer)
        .expect("test layer should carry ChunkLayer")
        .set_block(pos, BlockState::STONE);
    let mut overlay = JueBiTerrainOverlay::default();
    overlay.pending.push_back(TerrainModOp {
        pos,
        new_state: BlockState::MAGMA_BLOCK,
        anim_order: 0,
        restore_at_tick: 20,
    });
    overlay.pending.push_back(TerrainModOp {
        pos,
        new_state: BlockState::DEEPSLATE,
        anim_order: 1,
        restore_at_tick: 20,
    });
    app.insert_resource(CombatClock { tick: 10 });
    app.insert_resource(overlay);
    app.add_systems(Update, juebi_terrain_tick_system);

    app.update();

    {
        let overlay = app.world().resource::<JueBiTerrainOverlay>();
        assert_eq!(overlay.placed.len(), 1);
        assert_eq!(overlay.placed[0].original, BlockState::STONE);
    }
    {
        let layer_ref = app.world().get::<ChunkLayer>(layer).unwrap();
        assert_eq!(
            layer_ref.block(pos).map(|block| block.state),
            Some(BlockState::DEEPSLATE)
        );
    }

    app.world_mut().resource_mut::<CombatClock>().tick = 20;
    app.update();

    let layer_ref = app.world().get::<ChunkLayer>(layer).unwrap();
    assert_eq!(
        layer_ref.block(pos).map(|block| block.state),
        Some(BlockState::STONE)
    );
}
