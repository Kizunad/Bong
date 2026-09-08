// P2-21：公开 dormant 行为契约；私有系统与 cfg(test) accessor 留在同 crate mod_tests.rs。
use bong_server::cultivation::components::*;
use bong_server::cultivation::life_record::*;
use bong_server::cultivation::lifespan::*;
use bong_server::cultivation::meridian::severed::*;
use bong_server::fauna::daozhan::*;
use bong_server::npc::dormant::combat;
use bong_server::npc::dormant::*;
use bong_server::npc::faction::*;
use bong_server::npc::lifecycle::*;
use bong_server::qi_physics::constants::*;
use bong_server::qi_physics::{QiAccountId, WorldQiAccount};
use bong_server::world::dimension::*;
use bong_server::world::zone::*;
use valence::prelude::*;

fn zone() -> Zone {
    Zone {
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
    }
}

fn snapshot(char_id: &str, pos: DVec3) -> NpcDormantSnapshot {
    let cultivation = Cultivation {
        qi_current: 0.1,
        qi_max: 1.0,
        ..Default::default()
    };
    NpcDormantSnapshot {
        char_id: char_id.to_string(),
        archetype: NpcArchetype::Rogue,
        dimension: DimensionKind::Overworld,
        zone_name: DEFAULT_SPAWN_ZONE_NAME.to_string(),
        position: vec3_to_array(pos),
        schedule_seed: None,
        cultivation: cultivation.clone(),
        meridian_system: MeridianSystem::default(),
        meridian_severed: MeridianSeveredPermanent::default(),
        contamination: Contamination::default(),
        lifespan: NpcLifespan::new(0.0, 1_000.0),
        shared_lifespan: LifespanComponent::for_realm(cultivation.realm),
        lifespan_extension_ledger: LifespanExtensionLedger::default(),
        death_registry: DeathRegistry::new(char_id),
        life_record: LifeRecord::new(char_id),
        memory: None,
        player_reputation: None,
        faction: None,
        // 显式群体留空：走 effective_group 的 faction 派生回退路径（这里 faction=None ⇒
        // 群体 None ⇒ 不参战），顺带覆盖非破坏迁移分支。
        emergent_group: None,
        patrol: None,
        loot_table: None,
        guardian_relic: None,
        mimic_spider: None,
        tsy_hostile: None,
        tsy_sentinel: None,
        intent: DormantBehaviorIntent::Cultivate {
            zone: DEFAULT_SPAWN_ZONE_NAME.to_string(),
        },
        dormant_since_tick: 0,
        last_dormant_tick_processed: 0,
        initial_qi: 0.1,
        qi_ledger_net: 0.0,
        combat_dead_pending_release: false,
        pending_combat_winner: None,
    }
}

fn open_regular_meridians(snapshot: &mut NpcDormantSnapshot, count: usize) {
    for id in MeridianId::REGULAR.into_iter().take(count) {
        let meridian = snapshot.meridian_system.get_mut(id);
        meridian.opened = true;
    }
}

/// 造一个**敌对、满真元、不会自然老死**的战斗候选快照：给定 char_id / 派系 / 真元。
/// realm 拉到 Condense 让 condition_factor 由满血+满真元主导，战力 realm-monotonic。
fn combat_snapshot(
    char_id: &str,
    faction: FactionId,
    qi_current: f64,
    pos: DVec3,
) -> NpcDormantSnapshot {
    let cultivation = Cultivation {
        realm: Realm::Condense,
        qi_current,
        qi_max: 60.0,
        ..Default::default()
    };
    NpcDormantSnapshot {
        char_id: char_id.to_string(),
        archetype: NpcArchetype::Rogue,
        dimension: DimensionKind::Overworld,
        zone_name: DEFAULT_SPAWN_ZONE_NAME.to_string(),
        position: vec3_to_array(pos),
        schedule_seed: None,
        cultivation: cultivation.clone(),
        meridian_system: MeridianSystem::default(),
        meridian_severed: MeridianSeveredPermanent::default(),
        contamination: Contamination::default(),
        // 长寿命 + 0 起始年龄：本轮绝不自然老死，让"死人"唯一来源是战斗。
        lifespan: NpcLifespan::new(0.0, 1_000_000.0),
        shared_lifespan: LifespanComponent::for_realm(cultivation.realm),
        lifespan_extension_ledger: LifespanExtensionLedger::default(),
        death_registry: DeathRegistry::new(char_id),
        life_record: LifeRecord::new(char_id),
        memory: None,
        player_reputation: None,
        faction: Some(FactionMembership {
            faction_id: faction,
            rank: FactionRank::Disciple,
            reputation: Reputation::default(),
            lineage: None,
            mission_queue: MissionQueue::default(),
        }),
        // 显式群体留空：combat 候选默认走 faction 派生（Attack→0 / Defend→1），覆盖
        // 非破坏迁移路径；需要显式群体的测试单独 set 该字段。
        emergent_group: None,
        patrol: None,
        loot_table: None,
        guardian_relic: None,
        mimic_spider: None,
        tsy_hostile: None,
        tsy_sentinel: None,
        intent: DormantBehaviorIntent::Cultivate {
            zone: DEFAULT_SPAWN_ZONE_NAME.to_string(),
        },
        dormant_since_tick: 0,
        last_dormant_tick_processed: 0,
        initial_qi: qi_current,
        qi_ledger_net: 0.0,
        combat_dead_pending_release: false,
        pending_combat_winner: None,
    }
}

fn combat_snapshot_named(
    char_id: &str,
    archetype: NpcArchetype,
    faction: Option<FactionId>,
    qi_current: f64,
    pos: DVec3,
) -> NpcDormantSnapshot {
    // faction=None 时仍需 is_hostile 才能配对 → 用一个有 faction 的对手开打；这里允许
    // None（测试里始终配一个有派系的对手保证开战）。
    let mut snap = combat_snapshot(
        char_id,
        faction.unwrap_or(FactionId::Attack),
        qi_current,
        pos,
    );
    snap.archetype = archetype;
    snap.faction = faction.map(|faction_id| FactionMembership {
        faction_id,
        rank: FactionRank::Disciple,
        reputation: Reputation::default(),
        lineage: None,
        mission_queue: MissionQueue::default(),
    });
    snap
}

#[test]
fn dormant_store_clean_after_take_dirty() {
    let mut store = NpcDormantStore::default();
    store.insert(snapshot("npc_a", DVec3::new(10.0, 64.0, 10.0)));
    assert!(
        store.is_dirty(),
        "expected the store to be dirty right after an insert because the snapshot is unpersisted; it was clean"
    );

    assert!(
        store.take_dirty(),
        "expected take_dirty to return the prior dirty value (true) because an insert had occurred; it returned false"
    );
    assert!(
        !store.is_dirty(),
        "expected the store to be clean immediately after take_dirty consumed the flag; it still reported dirty"
    );
    assert!(
        !store.take_dirty(),
        "expected a second take_dirty with no intervening mutation to return false because the gate was already cleared; it returned true"
    );
}

#[test]
fn dormant_receipts_reject_malformed_stale_and_failed_authorization() {
    let mut store = NpcDormantStore::default();
    store.insert(snapshot("npc_receipt", DVec3::new(10.0, 64.0, 10.0)));
    let revision = store
        .begin_persistence()
        .expect("dirty store must start a HASH revision");
    let tx = store.persistence_receipt_sender();
    tx.send(bong_server::network::redis_bridge::RedisDeliveryReceipt {
        delivery_id: "not-a-revision".to_string(),
        outcome: Ok(()),
    })
    .unwrap();
    tx.send(bong_server::network::redis_bridge::RedisDeliveryReceipt {
        delivery_id: (revision + 1).to_string(),
        outcome: Ok(()),
    })
    .unwrap();
    tx.send(bong_server::network::redis_bridge::RedisDeliveryReceipt {
        delivery_id: revision.to_string(),
        outcome: Err("redis unavailable".to_string()),
    })
    .unwrap();
    store.apply_persistence_receipts();
    assert!(
        store.is_dirty(),
        "failed current receipt must re-arm the mutation"
    );
    assert_eq!(
        store.begin_persistence(),
        Some(revision),
        "retry must retain the failed revision instead of advancing it"
    );
}

#[test]
fn dormant_regen_moves_qi_through_ledger() {
    let mut snapshot = snapshot("npc_a", DVec3::new(10.0, 64.0, 10.0));
    open_regular_meridians(&mut snapshot, 1);
    let mut zones = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![zone()],
    };
    let mut ledger = WorldQiAccount::default();

    let transfer = apply_dormant_regen(&mut snapshot, &mut zones, &mut ledger)
        .expect("dormant regen should emit a transfer");

    assert_eq!(transfer.from, QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME));
    assert_eq!(transfer.to, QiAccountId::npc("npc_a"));
    assert!(snapshot.cultivation.qi_current > 0.1);
    assert!(
        zones
            .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
            .unwrap()
            .spirit_qi
            < 0.8
    );
    assert!(
        (snapshot.qi_ledger_net - transfer.amount).abs() < f64::EPSILON,
        "qi_ledger_net must audit the same amount as the ledger transfer"
    );
    assert!(
        !ledger.has_account(&QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME)),
        "Zone.spirit_qi is the sole environment owner; dormant regen must not leave a zone ledger shadow"
    );
    assert!(
        !ledger.has_account(&QiAccountId::npc("npc_a")),
        "snapshot Cultivation is the sole dormant actor owner; dormant regen must not leave an NPC ledger shadow"
    );
}

#[test]
fn dormant_regen_exempts_mundane_fauna_even_with_open_meridian_in_rich_zone() {
    // plan-mundane-fauna-v1 守恒红线（对称于 live 侧 qi_regen_excludes_mundane_fauna）：
    // 脱水凡兽即便开脉（sum_rate>0）、身处富灵区（普通 NPC 必吸），也**绝不**从 zone 吸真元。
    // 否则 snapshot.qi_current 被抽高、hydrate 带回 live 后死亡蒸发，破守恒。
    let mut snapshot = snapshot("npc_mundane_rabbit", DVec3::new(10.0, 64.0, 10.0));
    snapshot.archetype = NpcArchetype::Mundane;
    open_regular_meridians(&mut snapshot, 1); // sum_rate>0，普通 NPC 在此会吸
    let qi_before = snapshot.cultivation.qi_current;
    let mut zones = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![zone()],
    };
    let zone_qi_before = zones
        .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
        .unwrap()
        .spirit_qi;
    let mut ledger = WorldQiAccount::default();

    assert!(
        apply_dormant_regen(&mut snapshot, &mut zones, &mut ledger).is_none(),
        "凡兽脱水快照必须被 dormant regen 豁免（返回 None，无 QiTransfer）"
    );
    assert_eq!(
        snapshot.cultivation.qi_current, qi_before,
        "凡兽 qi_current 不得因 dormant regen 增长（无灵不吸气）"
    );
    assert_eq!(
        zones
            .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
            .unwrap()
            .spirit_qi,
        zone_qi_before,
        "凡兽豁免后 zone.spirit_qi 必须一分不动（守恒）"
    );
}

#[test]
fn dormant_regen_requires_open_meridian_flow() {
    let mut snapshot = snapshot("npc_a", DVec3::new(10.0, 64.0, 10.0));
    let mut zones = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![zone()],
    };
    let mut ledger = WorldQiAccount::default();

    assert!(apply_dormant_regen(&mut snapshot, &mut zones, &mut ledger).is_none());
    assert_eq!(snapshot.cultivation.qi_current, 0.1);
    assert_eq!(
        ledger.balance(&QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME)),
        0.0
    );
}

/// plan-zone-qi-economy-v1 P2：地板红线——zone_qi 在 (0, QI_NPC_ABSORB_FLOOR] 时
/// dormant NPC 必须完全放弃吸取，不能像玩家一样吃到地板以下。

#[test]
fn dormant_regen_stops_at_or_below_absorb_floor() {
    for zone_qi in [QI_NPC_ABSORB_FLOOR, 0.2, 0.05, 0.0] {
        let mut snapshot = snapshot("npc_a", DVec3::new(10.0, 64.0, 10.0));
        open_regular_meridians(&mut snapshot, 1);
        let mut z = zone();
        z.spirit_qi = zone_qi;
        let mut zones = ZoneRegistry {
            spatial_revision: 0,
            zones: vec![z],
        };
        let mut ledger = WorldQiAccount::default();

        assert!(
            apply_dormant_regen(&mut snapshot, &mut zones, &mut ledger).is_none(),
            "zone_qi={zone_qi} 已在/低于地板 {QI_NPC_ABSORB_FLOOR}，dormant regen 不应发生任何转移"
        );
        assert_eq!(
            snapshot.cultivation.qi_current, 0.1,
            "zone_qi={zone_qi} 时 NPC qi_current 不应变化"
        );
        assert_eq!(
            zones
                .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
                .unwrap()
                .spirit_qi,
            zone_qi,
            "zone_qi={zone_qi} 时 zone.spirit_qi 不应被 dormant regen 触碰"
        );
    }
}

/// 边界回归：zone_qi 略高于地板时 dormant regen 仍可发生，但写回后必须 >= 地板，
/// 一次 tick 的微量 drain 不会把 zone 拉穿地板（drain 本就远小于 0.31-0.3 的余量）。

#[test]
fn dormant_regen_never_dips_zone_below_absorb_floor() {
    let mut snapshot = snapshot("npc_a", DVec3::new(10.0, 64.0, 10.0));
    open_regular_meridians(&mut snapshot, 1);
    let mut z = zone();
    z.spirit_qi = QI_NPC_ABSORB_FLOOR + 0.01;
    let mut zones = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![z],
    };
    let mut ledger = WorldQiAccount::default();

    let transfer = apply_dormant_regen(&mut snapshot, &mut zones, &mut ledger)
        .expect("地板以上还有 0.01 余量，应发生一次转移");

    assert!(transfer.amount > 0.0);
    let zone_after = zones
        .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
        .unwrap()
        .spirit_qi;
    assert!(
        zone_after >= QI_NPC_ABSORB_FLOOR,
        "zone_qi 写回后 {zone_after} 不应低于地板 {QI_NPC_ABSORB_FLOOR}"
    );
}

/// 一批连续 tick（模拟长跑 dormant 批处理）不应把带回流 zone 压穿地板——
/// 即使反复调用，收敛点也应停在地板附近，绝不低于它。

#[test]
fn repeated_dormant_regen_ticks_converge_to_absorb_floor_without_crossing_it() {
    let mut snapshot = snapshot("npc_a", DVec3::new(10.0, 64.0, 10.0));
    open_regular_meridians(&mut snapshot, 1);
    // qi_max 拉大，避免 room 提前耗尽掩盖地板行为。
    snapshot.cultivation.qi_max = 1000.0;
    let mut z = zone();
    z.spirit_qi = 0.8;
    let mut zones = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![z],
    };
    let mut ledger = WorldQiAccount::default();

    for _ in 0..10_000 {
        if apply_dormant_regen(&mut snapshot, &mut zones, &mut ledger).is_none() {
            break;
        }
        let current = zones
            .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
            .unwrap()
            .spirit_qi;
        assert!(
            current >= QI_NPC_ABSORB_FLOOR,
            "批量 tick 期间 zone_qi 一度跌破地板：{current} < {QI_NPC_ABSORB_FLOOR}"
        );
    }
    let final_qi = zones
        .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
        .unwrap()
        .spirit_qi;
    assert!(
        final_qi >= QI_NPC_ABSORB_FLOOR,
        "长跑收敛后 zone_qi={final_qi} 不应低于地板 {QI_NPC_ABSORB_FLOOR}"
    );
}

/// plan-offscreen-war-v1 P9 war_multiplier 路径同样过地板——战事 zone 不给后门。

#[test]
fn dormant_regen_with_war_multiplier_still_respects_absorb_floor() {
    let mut snapshot = snapshot("npc_a", DVec3::new(10.0, 64.0, 10.0));
    open_regular_meridians(&mut snapshot, 1);
    let mut z = zone();
    z.spirit_qi = QI_NPC_ABSORB_FLOOR + 0.001;
    let mut zones = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![z],
    };
    let mut ledger = WorldQiAccount::default();

    // war_multiplier 拉到 10x，即便如此也不能把 zone 拉穿地板。
    let _ = apply_dormant_regen_with_multiplier(&mut snapshot, &mut zones, &mut ledger, 10.0);

    let zone_after = zones
        .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
        .unwrap()
        .spirit_qi;
    assert!(
        zone_after >= QI_NPC_ABSORB_FLOOR,
        "war_multiplier=10x 不应突破地板，实际 {zone_after}"
    );
}

#[test]
fn dormant_realm_label_uses_shared_schema_serializer() {
    let mut snapshot = snapshot("npc_a", DVec3::new(10.0, 64.0, 10.0));
    snapshot.cultivation.realm = Realm::Condense;

    assert_eq!(snapshot.realm_label(), "Condense");
}

#[test]
fn expired_dormant_npc_releases_qi_to_zone() {
    let mut snapshot = snapshot("npc_a", DVec3::new(10.0, 64.0, 10.0));
    snapshot.cultivation.qi_current = 0.4;
    let mut zones = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![zone()],
    };
    let mut ledger = WorldQiAccount::default();

    let outcome = release_dormant_qi_to_zone(&mut snapshot, &mut zones, &mut ledger)
        .expect("death release should commit typed settlement");
    let transfer = outcome
        .transfers
        .iter()
        .find(|transfer| transfer.to == QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME))
        .expect("available zone room should receive the entire death release");

    assert_eq!(transfer.from, QiAccountId::npc("npc_a"));
    assert_eq!(transfer.amount, 0.4);
    assert_eq!(outcome.zone_accepted, 0.4);
    assert_eq!(snapshot.cultivation.qi_current, 0.0);
    assert!(
        zones
            .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
            .unwrap()
            .spirit_qi
            > 0.8
    );
    assert!(
        !ledger.has_account(&QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME)),
        "Zone.spirit_qi is the sole environment owner; death release must not leave a zone ledger shadow"
    );
    assert!(
        !ledger.has_account(&QiAccountId::npc("npc_a")),
        "snapshot Cultivation is the sole dormant actor owner; death release must not leave an NPC ledger shadow"
    );
}

#[test]
fn daozhan_terminal_release_rolls_back_cultivation_when_second_owner_fails() {
    let mut snapshot = snapshot("npc_daozhan", DVec3::new(10.0, 64.0, 10.0));
    snapshot.cultivation.qi_current = 0.4;
    snapshot.tsy_hostile = Some(DormantTsyHostileSnapshot {
        family_id: "family-a".to_string(),
        zhinian_phase: None,
        zhinian_phase_entered_at_tick: None,
        fuya_aura: None,
        daoxiang_origin: None,
        daozhan: Some(DormantDaozhanSnapshot {
            state: DaoZhangState::default(),
            home_zone: DEFAULT_SPAWN_ZONE_NAME.to_string(),
            home_pos: snapshot.position,
            daozhan_qi: f64::NAN,
            origin_realm: None,
            behavior_queue: Vec::new(),
            current_behavior_ticks: 0,
        }),
    });
    let before = snapshot.clone();
    let mut zones = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![zone()],
    };
    let zones_before = zones.clone();
    let mut ledger = WorldQiAccount::default();

    release_dormant_qi_to_zone(&mut snapshot, &mut zones, &mut ledger)
        .expect_err("invalid Daozhan owner must roll back the earlier cultivation transfer");

    assert_eq!(snapshot.cultivation, before.cultivation);
    assert!(snapshot
        .tsy_hostile
        .as_ref()
        .and_then(|hostile| hostile.daozhan.as_ref())
        .is_some_and(|daozhan| daozhan.daozhan_qi.is_nan()));
    assert_eq!(zones, zones_before);
    assert_eq!(ledger.total(), 0.0);
    assert!(ledger.transfers().is_empty());
}

#[test]
fn death_qi_release_routes_zone_overflow_to_fixed_durable_pool() {
    let mut snapshot = snapshot("npc_a", DVec3::new(10.0, 64.0, 10.0));
    snapshot.cultivation.qi_current = 2.0;
    snapshot.cultivation.qi_max = 2.0;
    let mut full_zone = zone();
    full_zone.spirit_qi = 0.99;
    let mut zones = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![full_zone],
    };
    let mut ledger = WorldQiAccount::default();

    let outcome = release_dormant_qi_to_zone(&mut snapshot, &mut zones, &mut ledger)
        .expect("near-full zone plus fixed overflow should commit atomically");

    assert!((outcome.zone_accepted - 0.5).abs() < 1e-9);
    assert_eq!(snapshot.cultivation.qi_current, 0.0);
    assert!(
        (ledger.balance(&bong_server::qi_physics::qi_flow_overflow_account()) - 1.5).abs() < 1e-9,
        "zone overflow must be durably credited to the fixed qi_flow_overflow account"
    );
    assert!(
        !ledger.has_account(&QiAccountId::npc("npc_a")),
        "snapshot Cultivation is the sole dormant actor owner; no NPC shadow may remain"
    );
    assert!(
        !ledger.has_account(&QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME)),
        "Zone.spirit_qi is the sole environment owner; no Zone shadow may remain"
    );
}

#[test]
fn death_qi_release_with_missing_zone_routes_everything_to_fixed_overflow() {
    let mut snapshot = snapshot("npc_a", DVec3::new(10.0, 64.0, 10.0));
    snapshot.zone_name = "missing_zone".to_string();
    snapshot.cultivation.qi_current = 2.0;
    snapshot.cultivation.qi_max = 2.0;
    let mut zones = ZoneRegistry::default();
    zones.zones.clear();
    let mut ledger = WorldQiAccount::default();

    let outcome = release_dormant_qi_to_zone(&mut snapshot, &mut zones, &mut ledger)
        .expect("missing Zone is a supported settlement path through fixed overflow");

    assert_eq!(snapshot.cultivation.qi_current(), 0.0);
    assert_eq!(outcome.zone_accepted, 0.0);
    assert_eq!(outcome.overflow_credited, 2.0);
    assert_eq!(
        ledger.balance(&bong_server::qi_physics::qi_flow_overflow_account()),
        2.0
    );
    assert!(
        !ledger.has_account(&QiAccountId::npc("npc_a"))
            && !ledger.has_account(&QiAccountId::zone("missing_zone")),
        "missing-zone settlement must not synthesize actor or Zone mirrors"
    );
}

#[test]
fn death_qi_release_repays_negative_zone_without_clamping_signed_state() {
    let mut snapshot = snapshot("npc_a", DVec3::new(10.0, 64.0, 10.0));
    snapshot.cultivation.qi_current = 2.0;
    snapshot.cultivation.qi_max = 2.0;
    let mut negative_zone = zone();
    negative_zone.spirit_qi = -1.2;
    let mut zones = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![negative_zone],
    };
    let mut ledger = WorldQiAccount::default();

    let outcome = release_dormant_qi_to_zone(&mut snapshot, &mut zones, &mut ledger)
        .expect("signed negative Zone must accept a conservation release");

    assert_eq!(snapshot.cultivation.qi_current(), 0.0);
    assert!((zones.zones[0].spirit_qi - -1.16).abs() < 1e-9);
    assert_eq!(outcome.zone_accepted, 2.0);
    assert_eq!(outcome.overflow_credited, 0.0);
    assert_eq!(ledger.total(), 0.0);
}

#[test]
fn death_qi_release_failure_keeps_dormant_zone_ledger_and_audit_unchanged() {
    let mut snapshot = snapshot("npc_a", DVec3::new(10.0, 64.0, 10.0));
    snapshot.cultivation.qi_current = 2.0;
    snapshot.cultivation.qi_max = 2.0;
    let mut invalid_zone = zone();
    invalid_zone.spirit_qi = f64::NAN;
    let mut zones = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![invalid_zone],
    };
    let mut ledger = WorldQiAccount::default();

    let error = release_dormant_qi_to_zone(&mut snapshot, &mut zones, &mut ledger)
        .expect_err("invalid signed Zone owner must fail closed");

    assert!(matches!(error, QiFlowError::Physics(_)));
    assert_eq!(snapshot.cultivation.qi_current(), 2.0);
    assert!(zones.zones[0].spirit_qi.is_nan());
    assert_eq!(ledger.total(), 0.0);
    assert!(ledger.transfers().is_empty());
}

#[test]
fn death_qi_release_overflow_failure_keeps_all_physical_owners_unchanged() {
    let mut snapshot = snapshot("npc_a", DVec3::new(10.0, 64.0, 10.0));
    snapshot.cultivation.qi_current = 2.0;
    snapshot.cultivation.qi_max = 2.0;
    let mut full_zone = zone();
    full_zone.spirit_qi = 1.0;
    let mut zones = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![full_zone],
    };
    let mut ledger = WorldQiAccount::default();
    ledger
        .set_balance(
            bong_server::qi_physics::qi_flow_overflow_account(),
            f64::MAX,
        )
        .unwrap();

    release_dormant_qi_to_zone(&mut snapshot, &mut zones, &mut ledger)
        .expect_err("non-finite stable destination sum must fail closed");

    assert_eq!(snapshot.cultivation.qi_current(), 2.0);
    assert_eq!(zones.zones[0].spirit_qi, 1.0);
    assert_eq!(
        ledger.balance(&bong_server::qi_physics::qi_flow_overflow_account()),
        f64::MAX
    );
    assert!(ledger.transfers().is_empty());
}

#[test]
fn dormant_wander_uses_absolute_tick_salt() {
    let mut snapshot = snapshot("npc_a", DVec3::ZERO);
    snapshot.intent = DormantBehaviorIntent::Wander {
        drift_radius: 10_000.0,
    };
    let start = snapshot.position_vec();

    advance_dormant_position(&mut snapshot, 1200, 1200);
    let first = snapshot.position_vec();
    advance_dormant_position(&mut snapshot, 1200, 2400);
    let second = snapshot.position_vec();

    let straight_line_second = DVec3::new(
        start.x + (first.x - start.x) * 2.0,
        start.y,
        start.z + (first.z - start.z) * 2.0,
    );
    assert!(
        planar_distance(second, straight_line_second) > 1e-6,
        "wander angle must vary across absolute ticks instead of repeating one straight-line heading"
    );
}

#[test]
fn redis_payload_roundtrips_snapshot() {
    let mut store = NpcDormantStore::default();
    store.insert(snapshot("npc_a", DVec3::new(10.0, 64.0, 10.0)));

    let payloads = store.to_redis_hash_payloads().expect("serialize");
    assert_eq!(payloads[0].0, "npc_a");
    // 默认（非战死）快照：`combat_dead_pending_release=false` 必须被 skip_serializing_if
    // 省略，不写进 Redis payload（不膨胀，§10.1 #2）。
    assert!(
        !payloads[0].1.contains("combat_dead_pending_release"),
        "a normal (not combat-dead) snapshot must OMIT combat_dead_pending_release from its Redis payload (skip_serializing_if avoids snapshot bloat), but the field was present: {}",
        payloads[0].1
    );
    let decoded: NpcDormantSnapshot =
        serde_json::from_str(payloads[0].1.as_str()).expect("deserialize");
    assert_eq!(decoded.char_id, "npc_a");
    assert_eq!(decoded.position, [10.0, 64.0, 10.0]);
    assert!(
        !decoded.combat_dead_pending_release,
        "a roundtripped normal snapshot must decode combat_dead_pending_release=false; got true"
    );
}

#[test]
fn redis_payload_roundtrips_pending_release_flag() {
    // plan-offscreen-war-v1 P3 review-fix（守恒持久化安全）：被标记「战死待释放真元」的败者
    // 必须随 Redis 持久化——flag=true 往返不丢，server 重启后仍 pending-release（真元不丢、
    // 仍被 collect 跳过、绝不重复参战）。
    let mut snap = snapshot("trapped", DVec3::new(10.0, 64.0, 10.0));
    snap.combat_dead_pending_release = true;
    let payload = serde_json::to_string(&snap).expect("serialize flagged snapshot");
    assert!(
        payload.contains("combat_dead_pending_release"),
        "a flagged (combat-dead-pending-release) snapshot MUST serialize the field so it survives a Redis restart, but it was omitted: {payload}"
    );
    let decoded: NpcDormantSnapshot =
        serde_json::from_str(&payload).expect("deserialize flagged snapshot");
    assert!(
        decoded.combat_dead_pending_release,
        "flag=true must roundtrip through Redis JSON (restart safety: pending-release loser keeps its qi and stays out of combat); got false"
    );
}

#[test]
fn legacy_redis_snapshot_without_flag_defaults_to_false() {
    // 向后兼容：升级前写入 Redis 的旧快照没有 `combat_dead_pending_release` 字段，
    // `#[serde(default)]` 必须把它解码成 `false`（不 panic、不报错），否则升级即丢全部 dormant。
    // 用一份完整的旧快照 JSON（先 serialize 一个普通快照得到字段名，再手动删掉 flag 字段，
    // 这里因为默认快照本就不写该字段，直接复用其 payload 即「缺字段」样本）。
    let source = snapshot("legacy_npc", DVec3::new(5.0, 64.0, 5.0));
    let legacy_payload = serde_json::to_string(&source).expect("serialize");
    assert!(
        !legacy_payload.contains("combat_dead_pending_release"),
        "precondition: the synthesized legacy payload must lack the flag field"
    );
    let decoded: NpcDormantSnapshot = serde_json::from_str(&legacy_payload)
        .expect("legacy snapshot (no flag field) must deserialize via serde default");
    assert!(
        !decoded.combat_dead_pending_release,
        "a legacy Redis snapshot missing combat_dead_pending_release must default to false (serde default), so upgrades never lose or mis-flag dormant NPCs; got true"
    );
}

#[test]
fn legacy_redis_snapshot_without_tsy_sentinel_field_defaults_to_none() {
    // plan-tsy-sentinel-dormant-regression-v1 §P1：非破坏迁移——升级前写入 Redis 的旧
    // 快照没有 `tsy_sentinel` 字段（该字段是本 plan 新加的），`#[serde(default)]` 必须
    // 把它解码成 `None`（不 panic、不报错），即"退化为修复前的既有行为"（普通
    // overworld GuardianRelic），而不是升级即丢全部 dormant 快照。
    let source = snapshot("legacy_npc_no_sentinel", DVec3::new(5.0, 64.0, 5.0));
    let legacy_payload = serde_json::to_string(&source).expect("serialize");
    assert!(
        !legacy_payload.contains("tsy_sentinel"),
        "precondition: the synthesized legacy payload must lack the tsy_sentinel field \
         (skip_serializing_if omits None), got: {legacy_payload}"
    );
    let decoded: NpcDormantSnapshot = serde_json::from_str(&legacy_payload)
        .expect("legacy snapshot (no tsy_sentinel field) must deserialize via serde default");
    assert!(
        decoded.tsy_sentinel.is_none(),
        "a legacy Redis snapshot missing tsy_sentinel must default to None (serde default), \
         so upgrading the server binary never fails to load pre-existing dormant TSY sentinel \
         snapshots (they just degrade to the pre-fix plain-GuardianRelic hydrate path until \
         the next dehydrate cycle re-captures the field); got Some(..)"
    );
}

#[test]
fn combat_death_emits_no_relic_for_plain_factionless_rogue_pair() {
    // 一对**无派系、低境（Awaken）** Rogue。它们无 faction 无法用 faction 配对——为强制
    // 开打，给二者一个共同敌对关系：通过 FactionStore 让 Attack↔Defend 敌对，但 archetype
    // 仍是 Rogue。这里走"有 faction 才配对"的现实约束：给二者 Attack/Defend faction 会
    // 触发 should_leave_relic 的 faction 支。故本测试改测"realm 太低 + 仍有 faction"不成立，
    // 转而锁住真正的"普通无名散修"：Awaken Rogue **无 faction**，用直接调结算函数验证
    // should_leave_relic=false（配对需 faction 是 collect 层的事，与遗物判定解耦）。
    let plain = combat_snapshot_named(
        "nameless_rogue",
        NpcArchetype::Rogue,
        None,
        5.0,
        DVec3::new(10.0, 64.0, 10.0),
    );
    // 降到最低境，确保 realm 支也不成立。
    let mut plain = plain;
    plain.cultivation.realm = Realm::Awaken;
    assert!(
        !combat::should_leave_relic(&plain),
        "a nameless factionless Awaken-realm Rogue must NOT leave a relic; the combat settlement must skip relic emission for it"
    );
}
