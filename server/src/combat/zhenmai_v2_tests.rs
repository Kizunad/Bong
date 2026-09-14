#![allow(dead_code, unused_imports)]

use super::*;
use crate::combat::components::{WoundKind, Wounds};
use crate::cultivation::components::{ContamSource, MeridianSystem};
use crate::network::audio_event_emit::PlaySoundRecipeRequest;
use crate::skill::config::SkillConfig;
use valence::prelude::{App, Events, GameMode};

fn all_realms() -> [Realm; 6] {
    [
        Realm::Awaken,
        Realm::Induce,
        Realm::Condense,
        Realm::Solidify,
        Realm::Spirit,
        Realm::Void,
    ]
}

fn app_with_events() -> App {
    let mut app = App::new();
    app.insert_resource(CombatClock { tick: 100 });
    let mut dependencies = SkillMeridianDependencies::default();
    declare_meridian_dependencies(&mut dependencies);
    app.insert_resource(dependencies);
    app.add_event::<crate::combat::events::DefenseIntent>();
    app.add_event::<SkillXpGain>();
    app.add_event::<VfxEventRequest>();
    app.add_event::<PlaySoundRecipeRequest>();
    app.add_event::<LocalNeutralizeEvent>();
    app.add_event::<MeridianSeveredEvent>();
    app.add_event::<MeridianSeveredVoluntaryEvent>();
    app.add_event::<BackfireAmplificationActiveEvent>();
    app.add_event::<ZhenmaiSkillCastEvent>();
    app
}

fn caster(app: &mut App, realm: Realm, qi: f64) -> Entity {
    let mut meridians = MeridianSystem::default();
    for id in MeridianId::ALL {
        meridians.get_mut(id).opened = true;
    }
    app.world_mut()
        .spawn((
            Username("Azure".to_string()),
            Cultivation {
                realm,
                qi_current: qi,
                qi_max: qi.max(100.0),
                ..Default::default()
            },
            meridians,
            Wounds::default(),
            Contamination::default(),
            PracticeLog::default(),
            SkillBarBindings::default(),
            MeridianSeveredPermanent::default(),
        ))
        .id()
}

fn configure_sever_chain(
    app: &mut App,
    entity: Entity,
    meridian: MeridianId,
    kind: ZhenmaiAttackKind,
) {
    let mut store = SkillConfigStore::default();
    store.set_config(
        canonical_player_id("Azure").as_str(),
        SEVER_CHAIN_SKILL_ID,
        SkillConfig::new(BTreeMap::from([
            ("meridian_id".to_string(), serde_json::json!(meridian)),
            (
                "backfire_kind".to_string(),
                serde_json::json!(kind.as_str()),
            ),
        ])),
    );
    app.world_mut().insert_resource(store);
    assert_eq!(
        configured_sever_chain(app.world(), entity),
        Some((meridian, kind))
    );
}

fn mark_severed(app: &mut App, entity: Entity, meridian: MeridianId) {
    app.world_mut()
        .get_mut::<MeridianSeveredPermanent>(entity)
        .unwrap()
        .insert(meridian, SeveredSource::VoluntarySever, 99);
}

#[test]
fn resolve_parry_spends_qi_opens_defense_and_records_xp() {
    let mut app = app_with_events();
    let entity = caster(&mut app, Realm::Induce, 20.0);
    assert!(matches!(
        resolve_parry(app.world_mut(), entity, 0, None),
        CastResult::Started { .. }
    ));
    assert_eq!(
        app.world().get::<Cultivation>(entity).unwrap().qi_current,
        12.0
    );
    assert!(!app
        .world()
        .resource::<Events<crate::combat::events::DefenseIntent>>()
        .is_empty());
    assert!(!app.world().resource::<Events<SkillXpGain>>().is_empty());
}

#[test]
fn resolve_parry_rejects_insufficient_qi() {
    let mut app = app_with_events();
    let entity = caster(&mut app, Realm::Induce, 3.0);
    assert_eq!(
        resolve_parry(app.world_mut(), entity, 0, None),
        CastResult::Rejected {
            reason: CastRejectReason::QiInsufficient
        }
    );
}

#[test]
fn resolve_parry_rejects_declared_severed_meridian() {
    let mut app = app_with_events();
    let entity = caster(&mut app, Realm::Induce, 20.0);
    mark_severed(&mut app, entity, MeridianId::Lung);

    assert_eq!(
        resolve_parry(app.world_mut(), entity, 0, None),
        CastResult::Rejected {
            reason: CastRejectReason::MeridianSevered(Some(MeridianId::Lung))
        }
    );
}

#[test]
fn resolve_neutralize_removes_contam_with_realm_cap() {
    let mut app = app_with_events();
    let entity = caster(&mut app, Realm::Condense, 100.0);
    app.world_mut().entity_mut(entity).insert(Contamination {
        entries: vec![ContamSource {
            amount: 10.0,
            color: ColorKind::Insidious,
            meridian_id: Some(MeridianId::Lung.channel_id()),
            attacker_id: None,
            introduced_at: 1,
        }],
    });
    assert!(matches!(
        resolve_neutralize(app.world_mut(), entity, 0, None),
        CastResult::Started { .. }
    ));
    let contam = app.world().get::<Contamination>(entity).unwrap();
    assert_eq!(contam.entries[0].amount, 6.0);
}

#[test]
fn resolve_neutralize_keeps_other_meridian_contamination() {
    let mut app = app_with_events();
    let entity = caster(&mut app, Realm::Condense, 100.0);
    app.world_mut().entity_mut(entity).insert(Contamination {
        entries: vec![
            ContamSource {
                amount: 6.0,
                color: ColorKind::Insidious,
                meridian_id: Some(MeridianId::Lung.channel_id()),
                attacker_id: None,
                introduced_at: 1,
            },
            ContamSource {
                amount: 5.0,
                color: ColorKind::Turbid,
                meridian_id: Some(MeridianId::Heart.channel_id()),
                attacker_id: None,
                introduced_at: 1,
            },
        ],
    });

    assert!(matches!(
        resolve_neutralize(app.world_mut(), entity, 0, None),
        CastResult::Started { .. }
    ));
    let contam = app.world().get::<Contamination>(entity).unwrap();
    assert!(contam.entries.iter().any(|entry| entry.meridian_id
        == Some(MeridianId::Heart.channel_id())
        && entry.amount == 5.0));
}

#[test]
fn resolve_multipoint_inserts_active_component() {
    let mut app = app_with_events();
    let entity = caster(&mut app, Realm::Solidify, 100.0);
    assert!(matches!(
        resolve_multipoint(app.world_mut(), entity, 0, None),
        CastResult::Started { .. }
    ));
    let active = app.world().get::<MultiPointActive>(entity).unwrap();
    assert_eq!(active.points, 6);
}

#[test]
fn resolve_multipoint_rejects_declared_severed_meridian() {
    let mut app = app_with_events();
    let entity = caster(&mut app, Realm::Solidify, 100.0);
    mark_severed(&mut app, entity, MeridianId::Lung);

    assert_eq!(
        resolve_multipoint(app.world_mut(), entity, 0, None),
        CastResult::Rejected {
            reason: CastRejectReason::MeridianSevered(Some(MeridianId::Lung))
        }
    );
}

#[test]
fn resolve_harden_inserts_selected_meridian_component() {
    let mut app = app_with_events();
    let entity = caster(&mut app, Realm::Void, 100.0);
    assert!(matches!(
        resolve_harden(app.world_mut(), entity, 0, None),
        CastResult::Started { .. }
    ));
    let active = app.world().get::<MeridianHardenActive>(entity).unwrap();
    assert_eq!(active.meridians.len(), 2);
}

#[test]
fn resolve_sever_chain_writes_permanent_severed_and_amplification() {
    let mut app = app_with_events();
    let entity = caster(&mut app, Realm::Void, 200.0);
    configure_sever_chain(
        &mut app,
        entity,
        MeridianId::Du,
        ZhenmaiAttackKind::PhysicalCarrier,
    );
    assert_eq!(
        resolve_sever_chain(app.world_mut(), entity, 0, None),
        CastResult::Started {
            cooldown_ticks: SEVER_CHAIN_COOLDOWN_TICKS,
            anim_duration_ticks: 8
        }
    );
    assert!(app
        .world()
        .get::<MeridianSeveredPermanent>(entity)
        .unwrap()
        .is_severed(MeridianId::Du));
    assert!(app.world().get::<BackfireAmplification>(entity).is_some());
}

#[test]
fn resolve_sever_chain_below_spirit_still_severs_without_amplification() {
    let mut app = app_with_events();
    let entity = caster(&mut app, Realm::Condense, 50.0);
    configure_sever_chain(
        &mut app,
        entity,
        MeridianId::Ren,
        ZhenmaiAttackKind::RealYuan,
    );
    assert!(matches!(
        resolve_sever_chain(app.world_mut(), entity, 0, None),
        CastResult::Started { .. }
    ));
    assert_eq!(
        app.world().get::<Cultivation>(entity).unwrap().qi_current,
        0.0
    );
    assert!(app.world().get::<BackfireAmplification>(entity).is_none());
}

#[test]
fn resolve_sever_chain_below_spirit_still_requires_qi_cost() {
    let mut app = app_with_events();
    let entity = caster(&mut app, Realm::Condense, 49.0);
    configure_sever_chain(
        &mut app,
        entity,
        MeridianId::Ren,
        ZhenmaiAttackKind::RealYuan,
    );

    assert_eq!(
        resolve_sever_chain(app.world_mut(), entity, 0, None),
        CastResult::Rejected {
            reason: CastRejectReason::QiInsufficient
        }
    );
    assert!(!app
        .world()
        .get::<MeridianSeveredPermanent>(entity)
        .unwrap()
        .is_severed(MeridianId::Ren));
}

// ── qi conservation tests (BUG-QP-02) ──────────────────────────────────────

fn app_with_tick_systems() -> App {
    let mut app = App::new();
    app.insert_resource(CombatClock { tick: 100 });
    app.insert_resource(ZoneRegistry::fallback());
    app.add_event::<QiTransfer>();
    app.add_event::<MeridianHardenEvent>();
    app.add_systems(
        valence::prelude::Update,
        (multipoint_duration_tick, harden_duration_tick),
    );
    app
}

// ── spend_qi (world-path) ──────────────────────────────────────────────────

#[test]
fn spend_qi_emits_release_to_zone_when_registry_present() {
    let mut app = app_with_events();
    app.add_event::<QiTransfer>();
    // Use an empty zone so all 10.0 qi fits (spirit_qi=0.0 → zone_current=0.0, room=50.0).
    let mut registry = ZoneRegistry::fallback();
    registry.zones[0].spirit_qi = 0.0;
    app.insert_resource(registry);
    let entity = caster(&mut app, Realm::Condense, 50.0);
    app.world_mut().entity_mut(entity).insert((
        Position::new([0.0, 64.0, 0.0]),
        CurrentDimension(DimensionKind::Overworld),
    ));

    let ok = spend_qi(app.world_mut(), entity, 10.0);

    assert!(ok, "spend_qi should succeed when qi is sufficient");
    assert_eq!(
        app.world().get::<Cultivation>(entity).unwrap().qi_current,
        40.0,
        "qi_current should be reduced by the spent amount"
    );
    let events = app.world().resource::<Events<QiTransfer>>();
    let mut reader = events.get_reader();
    let transfers: Vec<_> = reader.read(events).collect();
    assert!(
        !transfers.is_empty(),
        "spend_qi must emit a QiTransfer event; none found — ledger leak detected"
    );
    assert!(
        transfers
            .iter()
            .any(|t| t.reason == QiTransferReason::ReleaseToZone),
        "QiTransfer reason must be ReleaseToZone; got {:?}",
        transfers.iter().map(|t| &t.reason).collect::<Vec<_>>()
    );
    // Total conserved: sum of all ReleaseToZone transfer amounts must equal 10.0.
    let total: f64 = transfers
        .iter()
        .filter(|t| t.reason == QiTransferReason::ReleaseToZone)
        .map(|t| t.amount)
        .sum();
    assert!(
        (total - 10.0).abs() < 1e-6,
        "total transferred qi must equal the drained amount (conservation); expected 10.0, got {total}"
    );
}

/// 部分饱和守恒（CodeRabbit #693）：zone 仅剩 room=10 但 spend 30 →
/// accepted=10 入 zone 账户、overflow=20 显式入 overflow 账户，绝不静默丢弃。
/// 两类 ReleaseToZone 之和必须 == 30（扣减量全额入账，不蒸发）。

#[test]
fn spend_qi_partial_saturation_routes_overflow_not_discard() {
    use crate::qi_physics::constants::QI_ZONE_UNIT_CAPACITY;
    use crate::qi_physics::ledger::QiAccountKind;
    let mut app = app_with_events();
    app.add_event::<QiTransfer>();
    // 期望值从常量推导（容量/spend 调整后测试不失真）。
    let initial_spirit_qi = 0.8;
    let spend_amount = 30.0;
    // room = (1.0 - spirit_qi) * CAP；accepted = min(room, spend)，overflow = 剩余。
    let expected_zone = ((1.0 - initial_spirit_qi) * QI_ZONE_UNIT_CAPACITY).min(spend_amount);
    let expected_overflow = spend_amount - expected_zone;
    let mut registry = ZoneRegistry::fallback();
    registry.zones[0].spirit_qi = initial_spirit_qi;
    app.insert_resource(registry);
    let entity = caster(&mut app, Realm::Condense, 50.0);
    app.world_mut().entity_mut(entity).insert((
        Position::new([0.0, 64.0, 0.0]),
        CurrentDimension(DimensionKind::Overworld),
    ));

    let ok = spend_qi(app.world_mut(), entity, spend_amount);
    assert!(ok, "spend_qi should succeed (qi 50 >= {spend_amount})");

    let events = app.world().resource::<Events<QiTransfer>>();
    let mut reader = events.get_reader();
    let releases: Vec<_> = reader
        .read(events)
        .filter(|t| t.reason == QiTransferReason::ReleaseToZone)
        .collect();
    let zone_sum: f64 = releases
        .iter()
        .filter(|t| t.to.kind == QiAccountKind::Zone)
        .map(|t| t.amount)
        .sum();
    let overflow_sum: f64 = releases
        .iter()
        .filter(|t| t.to.kind == QiAccountKind::Overflow)
        .map(|t| t.amount)
        .sum();
    assert!(
        (zone_sum - expected_zone).abs() < 1e-6,
        "zone 仅接受 room={expected_zone}（spirit_qi {initial_spirit_qi}→1.0），实际入 zone 账户 {zone_sum}"
    );
    assert!(
        (overflow_sum - expected_overflow).abs() < 1e-6,
        "饱和溢出 {expected_overflow} 必须显式入 overflow 账户而非丢弃，实际 {overflow_sum}（#693）"
    );
    let total: f64 = releases.iter().map(|t| t.amount).sum();
    assert!(
        (total - spend_amount).abs() < 1e-6,
        "守恒：ReleaseToZone 总量应 == spend {spend_amount}（zone {expected_zone} + overflow {expected_overflow}），实际 {total}（#693）"
    );
}

/// 最大边界（zone 满，room=0）：spirit_qi=1.0 时 spend_qi 全额走 overflow 账户。
/// 断言 Zone 账户得 0、Overflow 账户得 spend、总量 == spend，防 capped fallback 回归。

#[test]
fn spend_qi_zone_full_routes_all_to_overflow() {
    use crate::qi_physics::ledger::QiAccountKind;
    let mut app = app_with_events();
    app.add_event::<QiTransfer>();
    let spend_amount = 10.0;
    // spirit_qi=1.0 → zone_current=CAP, room=0：accepted=0 → 全额 overflow fallback。
    let mut registry = ZoneRegistry::fallback();
    registry.zones[0].spirit_qi = 1.0;
    app.insert_resource(registry);
    let entity = caster(&mut app, Realm::Condense, 50.0);
    app.world_mut().entity_mut(entity).insert((
        Position::new([0.0, 64.0, 0.0]),
        CurrentDimension(DimensionKind::Overworld),
    ));

    let ok = spend_qi(app.world_mut(), entity, spend_amount);
    assert!(ok, "spend_qi should succeed (qi 50 >= {spend_amount})");

    let events = app.world().resource::<Events<QiTransfer>>();
    let mut reader = events.get_reader();
    let releases: Vec<_> = reader
        .read(events)
        .filter(|t| t.reason == QiTransferReason::ReleaseToZone)
        .collect();
    let zone_sum: f64 = releases
        .iter()
        .filter(|t| t.to.kind == QiAccountKind::Zone)
        .map(|t| t.amount)
        .sum();
    let overflow_sum: f64 = releases
        .iter()
        .filter(|t| t.to.kind == QiAccountKind::Overflow)
        .map(|t| t.amount)
        .sum();
    assert!(
        zone_sum.abs() < 1e-6,
        "zone 满（room=0）应 0 入 zone 账户，实际 {zone_sum}"
    );
    assert!(
        (overflow_sum - spend_amount).abs() < 1e-6,
        "zone 满时全额 {spend_amount} 必须入 overflow 账户（非空转账），实际 {overflow_sum}"
    );
    let total: f64 = releases.iter().map(|t| t.amount).sum();
    assert!(
        (total - spend_amount).abs() < 1e-6,
        "守恒：zone 满时 ReleaseToZone 总量应 == spend {spend_amount}（全 overflow），实际 {total}"
    );
}

#[test]
fn spend_qi_falls_back_to_overflow_when_no_zone_registry() {
    let mut app = app_with_events();
    app.add_event::<QiTransfer>();
    // Deliberately do NOT insert ZoneRegistry.
    let entity = caster(&mut app, Realm::Condense, 30.0);

    let ok = spend_qi(app.world_mut(), entity, 5.0);

    assert!(ok);
    assert_eq!(
        app.world().get::<Cultivation>(entity).unwrap().qi_current,
        25.0
    );
    let events = app.world().resource::<Events<QiTransfer>>();
    let mut reader = events.get_reader();
    let transfers: Vec<_> = reader.read(events).collect();
    assert!(
        !transfers.is_empty(),
        "spend_qi must emit an overflow QiTransfer when no ZoneRegistry is present"
    );
    // The fallback uses ReleaseToZone reason on the overflow account.
    assert!(
        transfers
            .iter()
            .any(|t| t.reason == QiTransferReason::ReleaseToZone),
        "overflow transfer should still use ReleaseToZone reason"
    );
}

#[test]
fn spend_qi_returns_false_and_emits_nothing_when_insufficient() {
    let mut app = app_with_events();
    app.add_event::<QiTransfer>();
    app.insert_resource(ZoneRegistry::fallback());
    let entity = caster(&mut app, Realm::Condense, 3.0);

    let ok = spend_qi(app.world_mut(), entity, 10.0);

    assert!(!ok, "spend_qi should return false when qi is insufficient");
    assert_eq!(
        app.world().get::<Cultivation>(entity).unwrap().qi_current,
        3.0,
        "qi_current must be unchanged on failure"
    );
    let events = app.world().resource::<Events<QiTransfer>>();
    let mut reader = events.get_reader();
    let transfers: Vec<_> = reader.read(events).collect();
    assert!(
        transfers.is_empty(),
        "no QiTransfer event should be emitted when spend_qi fails"
    );
}

// ── multipoint_duration_tick (system-path) ─────────────────────────────────

#[test]
fn multipoint_tick_emits_qi_transfer_every_second() {
    let mut app = app_with_tick_systems();
    // CombatClock tick=100. Buff started at tick=0 → first drain at tick=TICKS_PER_SECOND.
    // We set CombatClock to exactly TICKS_PER_SECOND so the drain fires.
    app.insert_resource(CombatClock {
        tick: TICKS_PER_SECOND,
    });

    let entity = app
        .world_mut()
        .spawn((
            Cultivation {
                realm: Realm::Condense,
                qi_current: 50.0,
                qi_max: 100.0,
                ..Default::default()
            },
            MultiPointActive {
                started_at_tick: 0,
                expires_at_tick: TICKS_PER_SECOND * 10,
                points: 3,
                k_drain: 0.3,
                qi_per_second: 5.0,
                contact_count: 0,
                self_damage_per_contact: 0.0,
            },
            Position::new([0.0, 64.0, 0.0]),
            CurrentDimension(DimensionKind::Overworld),
        ))
        .id();

    app.update();

    let cultivation = app.world().get::<Cultivation>(entity).unwrap();
    assert!(
        cultivation.qi_current < 50.0,
        "qi_current must decrease after multipoint tick; was 50.0, now {}",
        cultivation.qi_current
    );

    let events = app.world().resource::<Events<QiTransfer>>();
    let mut reader = events.get_reader();
    let transfers: Vec<_> = reader.read(events).collect();
    assert!(
        !transfers.is_empty(),
        "multipoint_duration_tick must emit QiTransfer on each per-second drain; none found"
    );
    assert!(
        transfers
            .iter()
            .any(|t| t.reason == QiTransferReason::ReleaseToZone),
        "per-second drain must emit ReleaseToZone transfer; got {:?}",
        transfers.iter().map(|t| &t.reason).collect::<Vec<_>>()
    );
}

#[test]
fn multipoint_tick_emits_no_transfer_before_first_second() {
    let mut app = app_with_tick_systems();
    // Buff started at tick=0, clock at tick=5 (< TICKS_PER_SECOND) — no drain fires.
    app.insert_resource(CombatClock { tick: 5 });

    app.world_mut().spawn((
        Cultivation {
            realm: Realm::Condense,
            qi_current: 50.0,
            qi_max: 100.0,
            ..Default::default()
        },
        MultiPointActive {
            started_at_tick: 0,
            expires_at_tick: TICKS_PER_SECOND * 10,
            points: 3,
            k_drain: 0.3,
            qi_per_second: 5.0,
            contact_count: 0,
            self_damage_per_contact: 0.0,
        },
        Position::new([0.0, 64.0, 0.0]),
        CurrentDimension(DimensionKind::Overworld),
    ));

    app.update();

    let events = app.world().resource::<Events<QiTransfer>>();
    let mut reader = events.get_reader();
    let transfers: Vec<_> = reader.read(events).collect();
    assert!(
        transfers.is_empty(),
        "no QiTransfer should be emitted when drain has not fired yet; got {} events",
        transfers.len()
    );
}

// ── harden_duration_tick (system-path) ────────────────────────────────────

#[test]
fn harden_tick_emits_qi_transfer_every_second() {
    let mut app = app_with_tick_systems();
    app.insert_resource(CombatClock {
        tick: TICKS_PER_SECOND,
    });

    let entity = app
        .world_mut()
        .spawn((
            Cultivation {
                realm: Realm::Condense,
                qi_current: 60.0,
                qi_max: 100.0,
                ..Default::default()
            },
            MeridianHardenActive {
                started_at_tick: 0,
                expires_at_tick: TICKS_PER_SECOND * 10,
                meridians: vec![MeridianId::Lung],
                damage_multiplier: 0.5,
                qi_per_second: 4.0,
            },
            Position::new([0.0, 64.0, 0.0]),
            CurrentDimension(DimensionKind::Overworld),
        ))
        .id();

    app.update();

    let cultivation = app.world().get::<Cultivation>(entity).unwrap();
    assert!(
        cultivation.qi_current < 60.0,
        "qi_current must decrease after harden tick; was 60.0, now {}",
        cultivation.qi_current
    );

    let events = app.world().resource::<Events<QiTransfer>>();
    let mut reader = events.get_reader();
    let transfers: Vec<_> = reader.read(events).collect();
    assert!(
        !transfers.is_empty(),
        "harden_duration_tick must emit QiTransfer on each per-second drain; none found"
    );
    assert!(
        transfers
            .iter()
            .any(|t| t.reason == QiTransferReason::ReleaseToZone),
        "per-second drain must emit ReleaseToZone transfer; got {:?}",
        transfers.iter().map(|t| &t.reason).collect::<Vec<_>>()
    );
}

#[test]
fn harden_tick_emits_no_transfer_when_buff_expires() {
    let mut app = app_with_tick_systems();
    // Clock at the expiry tick — buff is removed, no drain fires.
    let expires = TICKS_PER_SECOND * 3;
    app.insert_resource(CombatClock { tick: expires });

    let entity = app
        .world_mut()
        .spawn((
            Cultivation {
                realm: Realm::Condense,
                qi_current: 60.0,
                qi_max: 100.0,
                ..Default::default()
            },
            MeridianHardenActive {
                started_at_tick: 0,
                expires_at_tick: expires,
                meridians: vec![MeridianId::Lung],
                damage_multiplier: 0.5,
                qi_per_second: 4.0,
            },
            Position::new([0.0, 64.0, 0.0]),
            CurrentDimension(DimensionKind::Overworld),
        ))
        .id();

    app.update();

    // Component should be removed (expired).
    assert!(
        app.world().get::<MeridianHardenActive>(entity).is_none(),
        "MeridianHardenActive must be removed when buff expires"
    );
    // qi_current unchanged — removal path skips drain.
    assert_eq!(
        app.world().get::<Cultivation>(entity).unwrap().qi_current,
        60.0,
        "qi must not be drained on expiry tick"
    );
    let events = app.world().resource::<Events<QiTransfer>>();
    let mut reader = events.get_reader();
    let transfers: Vec<_> = reader.read(events).collect();
    assert!(
        transfers.is_empty(),
        "no QiTransfer should be emitted on buff expiry (no drain); got {} events",
        transfers.len()
    );
}

// ─── plan-skill-anim-fidelity-v1 P5：粒子去复用回归锁 ─────────────────────────
//
// 去复用前 5 招挤在 3 个 `bong:jiemai_*` 上（multipoint 借 parry、harden 借
// neutralize），client 侧三者又全部指向剑气 SwordQiSlashPlayer。这组测试锁住
// 「每招发自己的 id + 自己的金脉色 + 绝不回退借用值」，断言值一律取自
// `network::skill_vfx_wiring` 的共享接线表（双端同源，防两端字符串各自漂移）。

/// 收集本 tick 发出的 SpawnParticle `(event_id, color)`。
fn emitted_particles(app: &App) -> Vec<(String, Option<String>)> {
    app.world()
        .resource::<Events<VfxEventRequest>>()
        .iter_current_update_events()
        .filter_map(|request| match &request.payload {
            VfxEventPayloadV1::SpawnParticle {
                event_id, color, ..
            } => Some((event_id.clone(), color.clone())),
            _ => None,
        })
        .collect()
}

/// 逐招对拍：恰好 1 条粒子、id/color 与共享接线表一致、且不是旧借用 id。
fn assert_emits_wired_particle(app: &App, skill: ZhenmaiSkillId, skill_id: &str) {
    let wiring = crate::network::skill_vfx_wiring::wiring_for(skill_id)
        .unwrap_or_else(|| panic!("{skill_id} 未登记进 P5_SKILL_VFX_WIRING 接线表"));

    let particles = emitted_particles(app);
    assert_eq!(
        particles.len(),
        1,
        "{skill_id} 应恰好发 1 条 SpawnParticle，实际 {particles:?}"
    );
    assert_eq!(
        particles[0].0, wiring.event_id,
        "{skill_id} 发出的粒子 event_id 与接线表不符（client 按表注册，不符即 bridgeMiss 静默无特效）"
    );
    assert_eq!(
        particles[0].1.as_deref(),
        Some(wiring.color),
        "{skill_id} 的粒子颜色应为金脉色系 {}，实际 {:?}",
        wiring.color,
        particles[0].1
    );
    assert_ne!(
        particles[0].0, wiring.legacy_event_id,
        "{skill_id} 回退到了 P5 之前的借用 id `{}`——去复用被撤销",
        wiring.legacy_event_id
    );
    assert_eq!(
        skill.particle_id(),
        wiring.event_id,
        "{skill_id} 的 ZhenmaiSkillId::particle_id() 与接线表不一致"
    );
}

#[test]
fn p5_parry_emits_bespoke_gold_pulse() {
    let mut app = app_with_events();
    let entity = caster(&mut app, Realm::Induce, 20.0);
    assert!(matches!(
        resolve_parry(app.world_mut(), entity, 0, None),
        CastResult::Started { .. }
    ));
    assert_emits_wired_particle(&app, ZhenmaiSkillId::Parry, PARRY_SKILL_ID);
}

#[test]
fn p5_neutralize_emits_bespoke_gold_pulse() {
    let mut app = app_with_events();
    let entity = caster(&mut app, Realm::Condense, 100.0);
    // 卸力中和需要有污染可卸——无污染时 resolver 直接 Rejected，粒子自然不发。
    app.world_mut().entity_mut(entity).insert(Contamination {
        entries: vec![ContamSource {
            amount: 10.0,
            color: ColorKind::Insidious,
            meridian_id: Some(MeridianId::Lung.channel_id()),
            attacker_id: None,
            introduced_at: 1,
        }],
    });
    assert!(matches!(
        resolve_neutralize(app.world_mut(), entity, 0, None),
        CastResult::Started { .. }
    ));
    assert_emits_wired_particle(&app, ZhenmaiSkillId::Neutralize, NEUTRALIZE_SKILL_ID);
}

#[test]
fn p5_multipoint_no_longer_borrows_parry_particle() {
    let mut app = app_with_events();
    let entity = caster(&mut app, Realm::Solidify, 100.0);
    assert!(matches!(
        resolve_multipoint(app.world_mut(), entity, 0, None),
        CastResult::Started { .. }
    ));
    assert_emits_wired_particle(&app, ZhenmaiSkillId::MultiPoint, MULTIPOINT_SKILL_ID);
    // 定向负向断言：multipoint 曾直接复用 PARRY_PARTICLE_ID 常量本身。
    assert_ne!(
        MULTIPOINT_PARTICLE_ID, PARRY_PARTICLE_ID,
        "multipoint 不得与 parry 共用粒子 id（去复用前正是这么写的）"
    );
}

#[test]
fn p5_harden_no_longer_borrows_neutralize_particle() {
    let mut app = app_with_events();
    let entity = caster(&mut app, Realm::Void, 100.0);
    assert!(matches!(
        resolve_harden(app.world_mut(), entity, 0, None),
        CastResult::Started { .. }
    ));
    assert_emits_wired_particle(&app, ZhenmaiSkillId::HardenMeridian, HARDEN_SKILL_ID);
    assert_ne!(
        HARDEN_PARTICLE_ID, NEUTRALIZE_PARTICLE_ID,
        "harden 不得与 neutralize 共用粒子 id（去复用前正是这么写的）"
    );
}

#[test]
fn p5_sever_chain_emits_bespoke_gold_pulse() {
    let mut app = app_with_events();
    let entity = caster(&mut app, Realm::Void, 200.0);
    configure_sever_chain(
        &mut app,
        entity,
        MeridianId::Du,
        ZhenmaiAttackKind::PhysicalCarrier,
    );
    assert!(matches!(
        resolve_sever_chain(app.world_mut(), entity, 0, None),
        CastResult::Started { .. }
    ));
    assert_emits_wired_particle(&app, ZhenmaiSkillId::SeverChain, SEVER_CHAIN_SKILL_ID);
    // sever 的旧 id 仍被 meridian_severed_emit 使用,所以这里锁的是"zhenmai 不再发它"。
    assert_ne!(
        SEVER_SNAP_PARTICLE_ID,
        crate::network::meridian_severed_emit::SEVER_FLASH_PARTICLE_ID,
        "主动断脉应发专属 id;被动断脉叙事才继续用 jiemai_sever_flash"
    );
}

/// P5 emit 架构统一 —— **端到端 emit-path 门**：真跑一次 `resolve_sever_chain` 施法，
/// 再跑真实音效系统 `emit_zhenmai_v2_audio_triggers`，断言实发的
/// `PlaySoundRecipeRequest.recipe_id` == 签名 `zhenmai_sever_crack`
/// （recipe id 从生产映射 `ZhenmaiSkillId::audio_recipe` 取，不另抄）。
///
/// 锁住整条链：cast 不发 `ZhenmaiSkillCastEvent` / 音效系统没读它 / 映射串味，任一处断都撞红。

#[test]
fn sever_chain_cast_emits_signature_recipe_end_to_end() {
    use crate::audio::implementation::AudioImplementationDedup;
    use crate::network::audio_trigger::emit_zhenmai_v2_audio_triggers;

    let mut app = app_with_events();
    app.init_resource::<AudioImplementationDedup>();
    app.add_systems(Update, emit_zhenmai_v2_audio_triggers);
    let entity = caster(&mut app, Realm::Void, 200.0);
    app.world_mut()
        .entity_mut(entity)
        .insert(Position::new([12.0, 64.0, -8.0]));
    configure_sever_chain(
        &mut app,
        entity,
        MeridianId::Du,
        ZhenmaiAttackKind::PhysicalCarrier,
    );

    assert!(matches!(
        resolve_sever_chain(app.world_mut(), entity, 0, None),
        CastResult::Started { .. }
    ));
    app.update();

    let emitted: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<PlaySoundRecipeRequest>>()
        .drain()
        .collect();
    let recipes: Vec<_> = emitted.iter().map(|e| e.recipe_id.as_str()).collect();
    assert_eq!(
        recipes,
        vec![ZhenmaiSkillId::SeverChain.audio_recipe()],
        "断脉施法应经真实 emit 系统实发签名 `{}`（不多不少一条），实际 {recipes:?}",
        ZhenmaiSkillId::SeverChain.audio_recipe()
    );
    assert_eq!(
        emitted[0].pos,
        Some([12, 64, -8]),
        "音源应落在施法时刻的施法者位置（事件 center）"
    );
}

/// **拒绝路径必须无声**（PR #1262 review 补门）：真元不足被 `Rejected` 的施法，
/// 既不许发 `ZhenmaiSkillCastEvent`，跑完真实音效系统后也不许有任何 `PlaySoundRecipeRequest`。
///
/// 「起手失败却响了招式音」是玩家可感知的错误反馈；只测成功路径锁不住它。

#[test]
fn rejected_cast_emits_no_audio_and_no_cast_event() {
    use crate::audio::implementation::AudioImplementationDedup;
    use crate::network::audio_trigger::emit_zhenmai_v2_audio_triggers;

    let mut app = app_with_events();
    app.init_resource::<AudioImplementationDedup>();
    app.add_systems(Update, emit_zhenmai_v2_audio_triggers);
    // 真元 3.0 < PARRY_QI_COST(8.0) —— 复用既有 `resolve_parry_rejects_insufficient_qi` 的前置
    let entity = caster(&mut app, Realm::Induce, 3.0);
    app.world_mut()
        .entity_mut(entity)
        .insert(Position::new([12.0, 64.0, -8.0]));

    assert_eq!(
        resolve_parry(app.world_mut(), entity, 0, None),
        CastResult::Rejected {
            reason: CastRejectReason::QiInsufficient
        }
    );
    app.update();

    let cast_events: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<ZhenmaiSkillCastEvent>>()
        .drain()
        .collect();
    assert!(
        cast_events.is_empty(),
        "被拒绝的施法不得发 ZhenmaiSkillCastEvent，实际 {cast_events:?}"
    );
    let recipes: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<PlaySoundRecipeRequest>>()
        .drain()
        .map(|event| event.recipe_id)
        .collect();
    assert!(
        recipes.is_empty(),
        "被拒绝的施法（真元不足）不得有任何招式音，实际 {recipes:?}"
    );
}

/// 五招施法各自发出带本招 id 的 `ZhenmaiSkillCastEvent`（音效解耦事件不串味）——
/// 与 `emit_zhenmai_v2_audio_triggers` 的 `zhenmai_skills_emit_their_mapped_recipes` 合起来
/// 覆盖「每招 → 各自 recipe」全链。

#[test]
fn each_skill_cast_sends_its_own_audio_cast_event() {
    /// cast 入口签名（与 `SkillRegistry::register` 收的函数指针同型）。
    type ResolveFn = fn(&mut bevy_ecs::world::World, Entity, u8, Option<Entity>) -> CastResult;

    let cases: [(ResolveFn, ZhenmaiSkillId); 5] = [
        (resolve_parry, ZhenmaiSkillId::Parry),
        (resolve_neutralize, ZhenmaiSkillId::Neutralize),
        (resolve_multipoint, ZhenmaiSkillId::MultiPoint),
        (resolve_harden, ZhenmaiSkillId::HardenMeridian),
        (resolve_sever_chain, ZhenmaiSkillId::SeverChain),
    ];
    for (resolve, expected) in cases {
        let mut app = app_with_events();
        let entity = caster(&mut app, Realm::Void, 200.0);
        if expected == ZhenmaiSkillId::SeverChain {
            configure_sever_chain(
                &mut app,
                entity,
                MeridianId::Du,
                ZhenmaiAttackKind::PhysicalCarrier,
            );
        }
        if expected == ZhenmaiSkillId::Neutralize {
            app.world_mut().entity_mut(entity).insert(Contamination {
                entries: vec![ContamSource {
                    amount: 10.0,
                    color: ColorKind::Insidious,
                    meridian_id: Some(MeridianId::Lung.channel_id()),
                    attacker_id: None,
                    introduced_at: 1,
                }],
            });
        }
        assert!(
            matches!(
                resolve(app.world_mut(), entity, 0, None),
                CastResult::Started { .. }
            ),
            "{expected:?} 应能起手（测试环境前置不足会让本断言先撞红）"
        );
        let events = app.world().resource::<Events<ZhenmaiSkillCastEvent>>();
        let skills: Vec<_> = events
            .iter_current_update_events()
            .map(|event| event.skill)
            .collect();
        assert_eq!(
            skills,
            vec![expected],
            "{expected:?} 施法应发且只发一条本招 ZhenmaiSkillCastEvent，实际 {skills:?}"
        );
    }
}

#[test]
fn p5_five_skills_have_pairwise_distinct_particles_in_gold_family() {
    let skills = [
        ZhenmaiSkillId::Parry,
        ZhenmaiSkillId::Neutralize,
        ZhenmaiSkillId::MultiPoint,
        ZhenmaiSkillId::HardenMeridian,
        ZhenmaiSkillId::SeverChain,
    ];
    let ids: std::collections::BTreeSet<&str> =
        skills.iter().map(|skill| skill.particle_id()).collect();
    assert_eq!(
        ids.len(),
        skills.len(),
        "5 招粒子 id 必须两两不同（旁观读招前提），实际 {ids:?}"
    );
    let colors: std::collections::BTreeSet<&str> =
        skills.iter().map(|skill| skill.particle_color()).collect();
    assert_eq!(
        colors.len(),
        skills.len(),
        "5 招粒子颜色必须两两不同（金脉明度阶梯），实际 {colors:?}"
    );
    // 全部落在 bong:zhenmai_ 前缀下——既是命名一致性,也让优先级前缀表自动命中 Important。
    for skill in skills {
        assert!(
            skill.particle_id().starts_with("bong:zhenmai_"),
            "{} 的粒子 id `{}` 不在 bong:zhenmai_ 家族前缀下,会掉出 Important 优先级档",
            skill.action(),
            skill.particle_id()
        );
    }
}

#[test]
fn p5_rejected_cast_emits_no_particle() {
    // 拒绝路径不得发粒子（否则玩家看到特效却没生效,是最糟的反馈错配）。
    let mut app = app_with_events();
    let entity = caster(&mut app, Realm::Induce, 1.0);
    assert!(matches!(
        resolve_parry(app.world_mut(), entity, 0, None),
        CastResult::Rejected { .. }
    ));
    assert!(emitted_particles(&app).is_empty(), "被拒绝的施放不得发粒子");
}
