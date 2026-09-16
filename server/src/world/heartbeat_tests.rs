#![allow(dead_code, unused_imports)]

use super::*;
use crate::world::dimension::DimensionKind;
use crate::worldgen::pseudo_vein::{decay_rate_per_tick, PSEUDO_VEIN_INITIAL_QI};
use valence::prelude::{App, DVec3};

fn zone(name: &str, x: f64, z: f64, spirit_qi: f64) -> Zone {
    Zone {
        name: name.to_string(),
        dimension: DimensionKind::Overworld,
        bounds: (
            DVec3::new(x - 50.0, 60.0, z - 50.0),
            DVec3::new(x + 50.0, 90.0, z + 50.0),
        ),
        spirit_qi,
        danger_level: 0,
        active_events: Vec::new(),
        patrol_anchors: vec![DVec3::new(x, 65.0, z)],
        blocked_tiles: Vec::new(),
        qi_equilibrium: 0.0,
        qi_inflow_per_min: 0.0,
    }
}

/// P3 §8.1 #5 — 与 [`zone`] 同款 fixture，但可配置 `danger_level`（`zone()` 恒
/// 硬编码 `danger_level: 0`，测不出 danger 加权效果）。
fn zone_with_danger(name: &str, x: f64, z: f64, spirit_qi: f64, danger_level: u8) -> Zone {
    Zone {
        danger_level,
        ..zone(name, x, z, spirit_qi)
    }
}

fn tsy_zone(name: &str, x: f64, z: f64, spirit_qi: f64) -> Zone {
    Zone {
        dimension: DimensionKind::Tsy,
        ..zone(name, x, z, spirit_qi)
    }
}

fn rhythm_context(loop_phase: PlayerLoopPhase, current_tick: u64) -> HeartbeatRhythmContext {
    HeartbeatRhythmContext {
        modifiers: season_event_modifiers(Season::Summer),
        loop_phase,
        current_tick,
    }
}

#[test]
fn pseudo_vein_removal_updates_spatial_revision_only_for_real_geometry_change() {
    let mut registry = ZoneRegistry {
        zones: vec![zone("pseudo_vein_test", 0.0, 0.0, 1.0)],
        spatial_revision: 41,
    };

    assert!(remove_runtime_pseudo_vein_zone(
        &mut registry,
        "pseudo_vein_test"
    ));
    assert_eq!(registry.spatial_revision, 42);
    assert!(!remove_runtime_pseudo_vein_zone(
        &mut registry,
        "pseudo_vein_test"
    ));
    assert_eq!(
        registry.spatial_revision, 42,
        "missing removal must not advance revision"
    );
    assert!(!remove_runtime_pseudo_vein_zone(
        &mut registry,
        "ordinary_zone"
    ));
    assert_eq!(
        registry.spatial_revision, 42,
        "non-pseudo names must be ignored"
    );
}

#[test]
fn heartbeat_override_suppress_and_force_are_stateful() {
    let mut heartbeat = WorldHeartbeat::default();
    heartbeat.apply_override(
        HeartbeatOverrideAction::Suppress,
        HeartbeatEventKind::BeastTide,
        "waste".to_string(),
        100,
        None,
        10,
    );
    assert!(heartbeat.is_suppressed(HeartbeatEventKind::BeastTide, "waste", 20));
    assert!(!heartbeat.is_suppressed(HeartbeatEventKind::BeastTide, "waste", 200));

    heartbeat.apply_override(
        HeartbeatOverrideAction::Force,
        HeartbeatEventKind::PseudoVein,
        "waste".to_string(),
        100,
        Some(0.9),
        10,
    );
    assert_eq!(heartbeat.forced_events.len(), 1);
    assert_eq!(heartbeat.forced_events[0].intensity, 0.9);
}

#[test]
fn override_command_parses_agent_contract() {
    let mut heartbeat = WorldHeartbeat::default();
    let command = Command {
        command_type: CommandType::HeartbeatOverride,
        target: "waste".to_string(),
        params: HashMap::from([
            ("action".to_string(), json!("accelerate")),
            ("event_type".to_string(), json!("beast_tide")),
            ("duration_ticks".to_string(), json!(6000)),
        ]),
    };

    apply_heartbeat_override_command(Some(&mut heartbeat), &command, 100).unwrap();

    assert_eq!(heartbeat.overrides.len(), 1);
    assert_eq!(
        heartbeat.overrides[0].action,
        HeartbeatOverrideAction::Accelerate
    );
    assert_eq!(
        heartbeat.overrides[0].event_kind,
        HeartbeatEventKind::BeastTide
    );
}

#[test]
fn pseudo_vein_omen_borrows_from_pending_pool_without_creating_qi() {
    let mut heartbeat = WorldHeartbeat::default();
    let mut zones = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![zone("waste", 0.0, 0.0, 0.1)],
    };
    let mut active_events = ActiveEventsResource::default();
    let mut qi_ledger = WorldQiAccount::default();
    qi_ledger
        .set_balance(pending_inflow_account(), 100.0)
        .expect("pending pool fixture should accept a finite balance");
    let physical_total_before = qi_ledger.total()
        + zones
            .zones
            .iter()
            .map(|zone| zone.spirit_qi * QI_ZONE_UNIT_CAPACITY)
            .sum::<f64>();
    let omen = WorldEventOmen {
        kind: OmenKind::PseudoVeinForming,
        zone_name: "waste".to_string(),
        target_player: None,
        origin: DVec3::new(10.0, 65.0, 10.0),
        intensity: 0.6,
        scheduled_at_tick: 0,
        fires_at_tick: 0,
        expires_at_tick: 200,
    };

    let transfer = spawn_pseudo_vein_from_omen(
        &mut heartbeat,
        &mut zones,
        &mut active_events,
        &mut qi_ledger,
        &omen,
        Season::Summer,
        200,
    )
    .expect("funded pending pool should allow the heartbeat pseudo-vein to spawn");

    assert_eq!(heartbeat.active_pseudo_vein_count(), 1);
    let pseudo_zone = zones
        .find_zone_by_name("pseudo_vein_heartbeat_0")
        .expect("successful omen should register one runtime pseudo-vein zone");
    assert_eq!(
        transfer.amount,
        0.6 * QI_ZONE_UNIT_CAPACITY,
        "expected omen intensity 0.6 to borrow the matching absolute qi amount"
    );
    assert_eq!(
        pseudo_zone.spirit_qi, 0.6,
        "expected dynamic zone field to reflect only the amount actually borrowed"
    );
    let physical_total_after = qi_ledger.total()
        + zones
            .zones
            .iter()
            .map(|zone| zone.spirit_qi * QI_ZONE_UNIT_CAPACITY)
            .sum::<f64>();
    assert_eq!(
        physical_total_after,
        physical_total_before,
        "expected pending-pool debit and external dynamic-zone credit to preserve the complete owner total"
    );
    assert_eq!(zones.find_zone_by_name("waste").unwrap().spirit_qi, 0.1);
}

#[test]
fn pseudo_vein_anchor_ignores_tsy_blueprint_zones() {
    let heartbeat = WorldHeartbeat::default();
    let zones = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![
            tsy_zone("tsy_daneng_01_deep", 0.0, 0.0, -0.95),
            zone("overworld_waste", 300.0, 0.0, 0.08),
        ],
    };

    let anchor = select_pseudo_vein_anchor(&zones, &heartbeat, 42)
        .expect("overworld zone should remain eligible for pseudo-vein anchor");

    assert_eq!(
        anchor.name, "overworld_waste",
        "TSY blueprint 常态负灵气不能抢走主世界伪灵脉锚点"
    );
}

#[test]
fn pseudo_vein_spawn_rejects_tsy_anchor_without_runtime_state() {
    let mut heartbeat = WorldHeartbeat::default();
    let mut zones = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![tsy_zone("tsy_daneng_01_shallow", 0.0, 0.0, -0.45)],
    };
    let mut active_events = ActiveEventsResource::default();
    let mut qi_ledger = WorldQiAccount::default();
    qi_ledger
        .set_balance(pending_inflow_account(), 100.0)
        .expect("pending pool fixture should accept a finite balance");
    let omen = WorldEventOmen {
        kind: OmenKind::PseudoVeinForming,
        zone_name: "tsy_daneng_01_shallow".to_string(),
        target_player: None,
        origin: DVec3::new(10.0, 65.0, 10.0),
        intensity: 0.6,
        scheduled_at_tick: 0,
        fires_at_tick: 0,
        expires_at_tick: 200,
    };

    assert!(
        spawn_pseudo_vein_from_omen(
            &mut heartbeat,
            &mut zones,
            &mut active_events,
            &mut qi_ledger,
            &omen,
            Season::Summer,
            200
        )
        .is_none(),
        "主世界 heartbeat 不应在 TSY blueprint 上创建伪灵脉 runtime zone"
    );
    assert_eq!(heartbeat.active_pseudo_vein_count(), 0);
    assert!(
        zones.find_zone_by_name("pseudo_vein_heartbeat_0").is_none(),
        "拒绝 TSY anchor 时不能泄漏 runtime pseudo-vein zone"
    );
}

#[test]
fn restored_pseudo_vein_records_rebuild_zone_and_advance_next_index() {
    let mut heartbeat = WorldHeartbeat::default();
    let mut zones = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![zone("waste", 0.0, 0.0, 0.1)],
    };
    let restored = heartbeat.restore_pseudo_vein_records(
        &mut zones,
        &[HeartbeatPseudoVeinRecord {
            zone_id: "pseudo_vein_heartbeat_7".to_string(),
            dimension: DimensionKind::Overworld,
            bounds_min: [-140.0, 60.0, -140.0],
            bounds_max: [160.0, 90.0, 160.0],
            danger_level: PSEUDO_VEIN_DANGER_LEVEL,
            active_events: Vec::new(),
            patrol_anchors: Vec::new(),
            center_xz: [10.0, 10.0],
            spawned_at_tick: 1_000,
            last_tick: 1_200,
            qi_current: 0.42,
            total_qi_consumed: 0.18,
            warning_sent: true,
            dissipated: false,
            season_at_spawn: crate::schema::pseudo_vein::PseudoVeinSeasonV1::Summer,
            observed_age_ticks: 200,
            pending_runtime_ticks: 0,
            pending_offline_ticks: 0,
            occupant_count: 0,
            eval_elapsed_ticks: 0,
            snapshot_wall: 0,
        }],
        2_000,
    );
    assert_eq!(
        restored, 1,
        "expected one valid pseudo-vein record to restore, actual {restored}"
    );
    assert_eq!(
        heartbeat.active_pseudo_vein_count(),
        1,
        "expected one active pseudo-vein because one record restored, actual {}",
        heartbeat.active_pseudo_vein_count()
    );
    let restored_state = heartbeat
        .active_pseudo_veins
        .get("pseudo_vein_heartbeat_7")
        .expect("hydrate must restore pseudo-vein lifecycle state");
    assert_eq!(
        restored_state.lifecycle.spawned_at, 1_800,
        "expected spawned_at 1800 because age 200 is rebased onto tick 2000, actual {}",
        restored_state.lifecycle.spawned_at
    );
    assert_eq!(
        restored_state.last_tick, 2_000,
        "expected last_tick 2000 because restart clock already represents the full age, actual {}",
        restored_state.last_tick
    );
    let restored_zone = zones
        .find_zone_by_name("pseudo_vein_heartbeat_7")
        .expect("hydrate must recreate runtime pseudo-vein zone");
    assert!(
        restored_zone
            .active_events
            .iter()
            .any(|event| event == EVENT_PSEUDO_VEIN),
        "restored zone must regain pseudo_vein active event even if old record omitted it"
    );
    assert_eq!(
        restored_zone.spirit_qi, 0.0,
        "expected lifecycle hydration to rebuild topology without creating qi; zones_runtime applies the persisted balance afterward"
    );

    let mut active_events = ActiveEventsResource::default();
    let mut qi_ledger = WorldQiAccount::default();
    qi_ledger
        .set_balance(pending_inflow_account(), 100.0)
        .expect("pending pool fixture should accept a finite balance");
    let omen = WorldEventOmen {
        kind: OmenKind::PseudoVeinForming,
        zone_name: "waste".to_string(),
        target_player: None,
        origin: DVec3::new(500.0, 65.0, 500.0),
        intensity: 0.6,
        scheduled_at_tick: 2_000,
        fires_at_tick: 2_000,
        expires_at_tick: 2_200,
    };
    assert!(spawn_pseudo_vein_from_omen(
        &mut heartbeat,
        &mut zones,
        &mut active_events,
        &mut qi_ledger,
        &omen,
        Season::Summer,
        2_000
    )
    .is_some());
    assert!(
        zones.find_zone_by_name("pseudo_vein_heartbeat_8").is_some(),
        "restore must advance next_pseudo_vein_index past restored heartbeat suffixes"
    );
}

#[test]
fn restored_pseudo_vein_preserves_age_across_restart_epoch_boundaries() {
    const OBSERVED_AGE: u64 = 200;
    for restart_tick in [0, 199, 200, 201] {
        let mut heartbeat = WorldHeartbeat::default();
        let mut zones = ZoneRegistry {
            spatial_revision: 0,
            zones: vec![zone("waste", 0.0, 0.0, 0.1)],
        };
        let restored = heartbeat.restore_pseudo_vein_records(
            &mut zones,
            &[heartbeat_pseudo_vein_record(0.42, false)],
            restart_tick,
        );
        assert_eq!(
            restored, 1,
            "expected one valid record at restart tick {restart_tick}, actual {restored}"
        );

        let state = heartbeat
            .active_pseudo_veins
            .get_mut("pseudo_vein_heartbeat_3")
            .expect("restart hydrate must restore pseudo-vein state");
        let effective_restart_tick = restart_tick.max(OBSERVED_AGE);
        let actual_age = state.last_tick.saturating_sub(state.lifecycle.spawned_at);
        assert_eq!(
            actual_age, OBSERVED_AGE,
            "expected age {OBSERVED_AGE} to survive restart tick {restart_tick}, actual {actual_age}"
        );
        assert_eq!(
            state.last_tick, effective_restart_tick,
            "expected effective tick {effective_restart_tick} to represent the full age at raw tick {restart_tick}, actual {}",
            state.last_tick
        );
        let qi_before = state.qi_current;

        let _ = state.advance(restart_tick.saturating_add(1), Vec::new());

        assert!(
            state.qi_current < qi_before,
            "expected first post-restart tick to decay qi from {qi_before}, actual {}",
            state.qi_current
        );
        assert_eq!(
            state.last_tick,
            effective_restart_tick.saturating_add(1),
            "expected effective last_tick to advance immediately after restart, actual {}",
            state.last_tick
        );
    }
}

#[test]
fn production_heartbeat_gate_restores_phase_and_counts_offline_wall_time() {
    let mut record = heartbeat_pseudo_vein_record(0.60, false);
    record.observed_age_ticks = 399;
    record.pending_runtime_ticks = 199;
    record.eval_elapsed_ticks = 199;
    record.snapshot_wall = 100;
    let mut heartbeat = WorldHeartbeat::default();
    let mut zones = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![zone("waste", 0.0, 0.0, 0.1)],
    };
    assert_eq!(
        heartbeat.restore_pseudo_vein_records_at_wall(
            &mut zones,
            std::slice::from_ref(&record),
            0,
            101,
        ),
        1,
        "expected one runtime to restore for production gate coverage"
    );
    assert_eq!(
        heartbeat.eval_elapsed_ticks(0),
        219,
        "expected 199 persisted phase ticks plus 20 offline wall ticks"
    );
    let zone_id = record.zone_id.clone();
    let before_qi = heartbeat
        .active_pseudo_veins
        .get(zone_id.as_str())
        .expect("restored state should exist")
        .qi_current;
    zones
        .find_zone_mut(zone_id.as_str())
        .expect("restored dynamic zone should exist")
        .spirit_qi = before_qi;
    let mut ledger = WorldQiAccount::default();
    ledger
        .set_balance(pending_inflow_account(), 10.0)
        .expect("pending pool fixture should initialize");
    let total_before = before_qi * QI_ZONE_UNIT_CAPACITY + ledger.total();

    let mut app = App::new();
    app.insert_resource(heartbeat);
    app.insert_resource(CultivationClock { tick: 0 });
    app.insert_resource(ActiveEventsResource::default());
    app.insert_resource(zones);
    app.insert_resource(ledger);
    app.add_event::<EventChainTrigger>();
    app.add_event::<QiTransfer>();
    app.add_systems(Update, heartbeat_tick);
    app.update();

    let heartbeat = app.world().resource::<WorldHeartbeat>();
    let after_qi = heartbeat
        .active_pseudo_veins
        .get(zone_id.as_str())
        .expect("runtime should remain active after a short catch-up")
        .qi_current;
    assert!(
        after_qi < before_qi,
        "expected first production heartbeat to apply persisted+offline elapsed, before {before_qi}, actual {after_qi}"
    );
    assert_eq!(
        heartbeat.eval_elapsed_ticks(0),
        0,
        "expected restored heartbeat phase carry to clear after the due evaluation"
    );
    let zones = app.world().resource::<ZoneRegistry>();
    let zone_after = zones
        .find_zone_by_name(zone_id.as_str())
        .expect("restored dynamic zone should remain after catch-up")
        .spirit_qi;
    let ledger_after = app.world().resource::<WorldQiAccount>();
    assert!(
        (zone_after * QI_ZONE_UNIT_CAPACITY + ledger_after.total() - total_before).abs() < 1e-9,
        "expected catch-up decay to move qi from the external Zone owner into the pending pool"
    );
    assert!(
        !ledger_after.has_account(&QiAccountId::zone(zone_id.as_str())),
        "catch-up must not recreate a zone:* ledger mirror"
    );
}

#[test]
fn restored_pseudo_vein_preserves_warning_then_dissipation_boundaries() {
    let mut heartbeat = WorldHeartbeat::default();
    let mut zones = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![zone("waste", 0.0, 0.0, 0.1)],
    };
    let decay_per_tick = decay_rate_per_tick(0);
    let restored = heartbeat.restore_pseudo_vein_records(
        &mut zones,
        &[heartbeat_pseudo_vein_record(decay_per_tick * 1.5, false)],
        0,
    );
    assert_eq!(
        restored, 1,
        "expected low-qi pseudo-vein fixture to restore, actual {restored}"
    );
    let state = heartbeat
        .active_pseudo_veins
        .get_mut("pseudo_vein_heartbeat_3")
        .expect("low-qi restart fixture must exist");

    let warning = state.advance(1, Vec::new());
    assert!(
        warning.warning_threshold_crossed,
        "expected first post-restart tick to cross warning threshold, actual {}",
        warning.warning_threshold_crossed
    );
    assert!(
        warning.dissipate_event.is_none(),
        "expected warning tick to retain positive qi, actual dissipate_event={:?}",
        warning.dissipate_event
    );

    let dissipated = state.advance(2, Vec::new());
    assert!(
        dissipated.dissipate_event.is_some(),
        "expected second post-restart tick to dissipate exhausted vein, actual {:?}",
        dissipated.dissipate_event
    );
    assert!(
        state.dissipated,
        "expected runtime state to enter dissipated after qi reaches zero, actual {}",
        state.dissipated
    );
}

#[test]
fn heartbeat_tick_keeps_pseudo_vein_state_zone_and_ledger_in_lockstep() {
    let mut heartbeat = WorldHeartbeat {
        eval_interval_ticks: 1,
        ..Default::default()
    };
    let mut zones = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![zone("waste", 0.0, 0.0, 0.1)],
    };
    let mut active_events = ActiveEventsResource::default();
    let mut qi_ledger = WorldQiAccount::default();
    qi_ledger
        .set_balance(pending_inflow_account(), 100.0)
        .expect("pending pool fixture should accept a finite balance");
    let omen = WorldEventOmen {
        kind: OmenKind::PseudoVeinForming,
        zone_name: "waste".to_string(),
        target_player: None,
        origin: DVec3::new(10.0, 65.0, 10.0),
        intensity: 0.6,
        scheduled_at_tick: 0,
        fires_at_tick: 0,
        expires_at_tick: 200,
    };
    spawn_pseudo_vein_from_omen(
        &mut heartbeat,
        &mut zones,
        &mut active_events,
        &mut qi_ledger,
        &omen,
        Season::Summer,
        0,
    )
    .expect("funded pending pool should spawn the dynamic pseudo-vein fixture");
    let dynamic_zone_qi = zones
        .find_zone_by_name("pseudo_vein_heartbeat_0")
        .expect("spawned pseudo-vein zone should exist")
        .spirit_qi;
    let total_before = dynamic_zone_qi * QI_ZONE_UNIT_CAPACITY + qi_ledger.total();

    let mut app = App::new();
    app.insert_resource(heartbeat);
    app.insert_resource(zones);
    app.insert_resource(active_events);
    app.insert_resource(CultivationClock { tick: 1 });
    app.insert_resource(qi_ledger);
    app.add_event::<EventChainTrigger>();
    app.add_event::<QiTransfer>();
    app.add_systems(Update, heartbeat_tick);

    app.update();

    let heartbeat = app.world().resource::<WorldHeartbeat>();
    let zones = app.world().resource::<ZoneRegistry>();
    let record = heartbeat
        .active_pseudo_vein_records(zones)
        .into_iter()
        .next()
        .expect("one active pseudo-vein should remain after a single decay tick");
    let zone = zones
        .find_zone_by_name(record.zone_id.as_str())
        .expect("active pseudo-vein record must retain its dynamic zone");
    let ledger = app.world().resource::<WorldQiAccount>();
    let zone_account = QiAccountId::zone(record.zone_id.as_str());
    assert!(
        record.qi_current < 0.6,
        "expected lifecycle qi to decay below 0.6 after one tick, actual {}",
        record.qi_current
    );
    assert!(
        (record.qi_current - zone.spirit_qi).abs() < 1e-12,
        "expected lifecycle and zone qi to match, state={} zone={}",
        record.qi_current,
        zone.spirit_qi
    );
    assert!(
        !ledger.has_account(&zone_account),
        "the dynamic Zone field is the physical owner; no zone:* ledger mirror may remain"
    );
    assert!(
        (zone.spirit_qi * QI_ZONE_UNIT_CAPACITY + ledger.total() - total_before).abs() < 1e-9,
        "continuous pseudo-vein decay must conserve external Zone qi plus stable ledger pools"
    );
    assert!(
        ledger.transfers().iter().any(|transfer| {
            transfer.from == zone_account
                && transfer.to == pending_inflow_account()
                && transfer.reason == QiTransferReason::PseudoVeinSettle
                && transfer.amount > 0.0
        }),
        "expected one auditable PseudoVeinSettle transfer for the decay delta"
    );
}

#[test]
fn post_update_sync_tracks_external_zone_change_without_rewriting_ledger() {
    let mut heartbeat = WorldHeartbeat::default();
    let mut zones = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![zone("waste", 0.0, 0.0, 0.1)],
    };
    let mut active_events = ActiveEventsResource::default();
    let mut qi_ledger = WorldQiAccount::default();
    qi_ledger
        .set_balance(pending_inflow_account(), 100.0)
        .expect("pending pool fixture should accept a finite balance");
    let omen = WorldEventOmen {
        kind: OmenKind::PseudoVeinForming,
        zone_name: "waste".to_string(),
        target_player: None,
        origin: DVec3::new(10.0, 65.0, 10.0),
        intensity: 0.6,
        scheduled_at_tick: 0,
        fires_at_tick: 0,
        expires_at_tick: 200,
    };
    spawn_pseudo_vein_from_omen(
        &mut heartbeat,
        &mut zones,
        &mut active_events,
        &mut qi_ledger,
        &omen,
        Season::Summer,
        0,
    )
    .expect("funded pending pool should spawn the dynamic pseudo-vein fixture");
    zones
        .find_zone_mut("pseudo_vein_heartbeat_0")
        .expect("spawned dynamic zone must exist")
        .spirit_qi = 0.55;
    let zone_account = QiAccountId::zone("pseudo_vein_heartbeat_0");
    let ledger_total_before = qi_ledger.total();
    qi_ledger.push_transfer_audit(
        QiTransfer::new(
            zone_account.clone(),
            QiAccountId::player("external-cultivator"),
            0.05 * QI_ZONE_UNIT_CAPACITY,
            QiTransferReason::CultivationRegen,
        )
        .expect("external cultivation audit fixture should be valid"),
    );

    let mut app = App::new();
    app.insert_resource(heartbeat);
    app.insert_resource(zones);
    app.insert_resource(active_events);
    app.insert_resource(CultivationClock { tick: 1 });
    app.insert_resource(qi_ledger);
    app.add_event::<EventChainTrigger>();
    app.add_event::<QiTransfer>();
    app.add_systems(Update, heartbeat_tick);
    app.add_systems(PostUpdate, sync_active_pseudo_vein_state_system);

    app.update();

    let heartbeat = app.world().resource::<WorldHeartbeat>();
    let state = heartbeat
        .active_pseudo_veins
        .get("pseudo_vein_heartbeat_0")
        .expect("non-due heartbeat tick must retain active runtime");
    assert_eq!(
        state.qi_current, 0.55,
        "expected PostUpdate sync to observe external zone drain before next heartbeat eval"
    );
    let ledger = app.world().resource::<WorldQiAccount>();
    assert!(
        !ledger.has_account(&zone_account),
        "frame-end lifecycle sync must not create a Zone ledger mirror"
    );
    assert_eq!(
        ledger.total(),
        ledger_total_before,
        "expected frame-end lifecycle sync to preserve stable ledger total, actual {}",
        ledger.total()
    );
    assert!(
        ledger
            .transfers()
            .iter()
            .any(|transfer| transfer.reason == QiTransferReason::CultivationRegen),
        "expected external zone consumption to retain its audit trail"
    );
    assert!(
        ledger
            .transfers()
            .iter()
            .all(|transfer| transfer.reason != QiTransferReason::PseudoVeinSettle),
        "expected non-due heartbeat tick not to apply lifecycle decay"
    );
}

#[test]
fn restored_pseudo_vein_first_tick_returns_dynamic_zone_balance_to_pending_pool() {
    let mut heartbeat = WorldHeartbeat {
        eval_interval_ticks: 1,
        ..Default::default()
    };
    let mut zones = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![zone("waste", 0.0, 0.0, 0.1)],
    };
    let mut record = heartbeat_pseudo_vein_record(decay_rate_per_tick(0) * 0.5, false);
    record.total_qi_consumed = PSEUDO_VEIN_INITIAL_QI - record.qi_current;
    assert_eq!(
        heartbeat.restore_pseudo_vein_records(&mut zones, &[record.clone()], 0),
        1,
        "expected one near-expiry pseudo-vein fixture to restore"
    );
    let restored_zone = zones
        .find_zone_mut(record.zone_id.as_str())
        .expect("restored pseudo-vein topology must exist before runtime overlay");
    restored_zone.spirit_qi = record.qi_current;

    let zone_account = QiAccountId::zone(record.zone_id.as_str());
    let zone_absolute = restored_zone.spirit_qi * QI_ZONE_UNIT_CAPACITY;
    let qi_ledger = WorldQiAccount::default();
    let total_before = zone_absolute;

    let mut app = App::new();
    app.insert_resource(heartbeat);
    app.insert_resource(zones);
    app.insert_resource(ActiveEventsResource::default());
    app.insert_resource(CultivationClock { tick: 1 });
    app.insert_resource(qi_ledger);
    app.add_event::<EventChainTrigger>();
    app.add_event::<QiTransfer>();
    app.add_systems(Update, heartbeat_tick);

    app.update();

    let heartbeat = app.world().resource::<WorldHeartbeat>();
    assert_eq!(
        heartbeat.active_pseudo_vein_count(),
        0,
        "expected first post-restart tick to finish the exhausted pseudo-vein"
    );
    let zones = app.world().resource::<ZoneRegistry>();
    assert_eq!(
        zones
            .find_zone_by_name(record.zone_id.as_str())
            .expect("chain reaction is not registered in this focused test, so zone remains")
            .spirit_qi,
        0.0,
        "expected settlement to drain the ephemeral zone before it can be removed"
    );
    let qi_ledger = app.world().resource::<WorldQiAccount>();
    assert!(
        !qi_ledger.has_account(&zone_account),
        "expected no zone:* mirror after settlement"
    );
    assert_eq!(
        qi_ledger.balance(&pending_inflow_account()),
        zone_absolute,
        "expected every remaining absolute qi unit to return to the pending pool"
    );
    assert_eq!(
        qi_ledger.total(),
        total_before,
        "expected restart tick plus dissipation settlement to preserve total ledger qi"
    );
    assert!(
        qi_ledger.transfers().iter().any(|transfer| {
            transfer.from == zone_account
                && transfer.to == pending_inflow_account()
                && transfer.reason == QiTransferReason::PseudoVeinSettle
                && transfer.amount == zone_absolute
        }),
        "expected one auditable PseudoVeinSettle transfer for the full dynamic-zone balance"
    );
}

#[test]
fn restored_pseudo_vein_can_persist_and_restore_again_without_losing_age() {
    let mut heartbeat = WorldHeartbeat::default();
    let mut zones = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![zone("waste", 0.0, 0.0, 0.1)],
    };
    let restored = heartbeat.restore_pseudo_vein_records(
        &mut zones,
        &[heartbeat_pseudo_vein_record(0.42, false)],
        0,
    );
    assert_eq!(
        restored, 1,
        "expected first restart to restore one record, actual {restored}"
    );
    let state = heartbeat
        .active_pseudo_veins
        .get_mut("pseudo_vein_heartbeat_3")
        .expect("first restart fixture must exist");
    let _ = state.advance(5, Vec::new());
    let qi_after_five_ticks = state.qi_current;
    zones
        .find_zone_mut("pseudo_vein_heartbeat_3")
        .expect("first restart dynamic zone must exist")
        .spirit_qi = qi_after_five_ticks;
    let records = heartbeat.active_pseudo_vein_records(&zones);
    assert_eq!(
        records.len(),
        1,
        "expected active restored vein to be persistable, actual record count {}",
        records.len()
    );
    assert_eq!(
        records[0]
            .last_tick
            .saturating_sub(records[0].spawned_at_tick),
        205,
        "expected persisted age 205 after five new ticks, actual {}",
        records[0]
            .last_tick
            .saturating_sub(records[0].spawned_at_tick)
    );

    let mut second_heartbeat = WorldHeartbeat::default();
    let mut second_zones = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![zone("waste", 0.0, 0.0, 0.1)],
    };
    let restored_again =
        second_heartbeat.restore_pseudo_vein_records(&mut second_zones, records.as_slice(), 0);
    assert_eq!(
        restored_again, 1,
        "expected persisted runtime to restore a second time, actual {restored_again}"
    );
    let second_state = second_heartbeat
        .active_pseudo_veins
        .get("pseudo_vein_heartbeat_3")
        .expect("second restart fixture must exist");
    assert_eq!(
        second_state
            .last_tick
            .saturating_sub(second_state.lifecycle.spawned_at),
        205,
        "expected second restart to retain age 205, actual {}",
        second_state
            .last_tick
            .saturating_sub(second_state.lifecycle.spawned_at)
    );
    assert_eq!(
        second_state.qi_current, qi_after_five_ticks,
        "expected second restart to retain qi {qi_after_five_ticks}, actual {}",
        second_state.qi_current
    );
}

fn heartbeat_pseudo_vein_record(qi_current: f64, warning_sent: bool) -> HeartbeatPseudoVeinRecord {
    HeartbeatPseudoVeinRecord {
        zone_id: "pseudo_vein_heartbeat_3".to_string(),
        dimension: DimensionKind::Overworld,
        bounds_min: [-10.0, 60.0, -10.0],
        bounds_max: [10.0, 90.0, 10.0],
        danger_level: PSEUDO_VEIN_DANGER_LEVEL,
        active_events: vec![EVENT_PSEUDO_VEIN.to_string()],
        patrol_anchors: Vec::new(),
        center_xz: [0.0, 0.0],
        spawned_at_tick: 1_000,
        last_tick: 1_200,
        qi_current,
        total_qi_consumed: 0.18,
        warning_sent,
        dissipated: false,
        season_at_spawn: crate::schema::pseudo_vein::PseudoVeinSeasonV1::Summer,
        observed_age_ticks: 200,
        pending_runtime_ticks: 0,
        pending_offline_ticks: 0,
        occupant_count: 0,
        eval_elapsed_ticks: 0,
        snapshot_wall: 0,
    }
}

#[test]
fn accelerate_intensity_override_controls_queued_omen_strength() {
    let mut heartbeat = WorldHeartbeat::default();
    heartbeat
        .low_qi_ticks_by_zone
        .insert("hungry".to_string(), BEAST_TIDE_LOW_QI_REQUIRED_TICKS);
    heartbeat.apply_override(
        HeartbeatOverrideAction::Accelerate,
        HeartbeatEventKind::BeastTide,
        "hungry".to_string(),
        50_000,
        Some(0.42),
        0,
    );
    let zones = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![zone("hungry", 0.0, 0.0, 0.1)],
    };
    let npc_registry = NpcRegistry {
        counts_by_zone: HashMap::from([("hungry".to_string(), 6)]),
        ..Default::default()
    };

    maybe_queue_beast_tide(
        &mut heartbeat,
        &zones,
        Some(&npc_registry),
        &ActiveEventsResource::default(),
        rhythm_context(PlayerLoopPhase::DeepGathering, 20_000),
        None,
    );

    assert_eq!(heartbeat.pending_omens.len(), 1);
    assert_eq!(
        heartbeat.pending_omens[0].intensity, 0.42,
        "accelerate intensity_override should drive queued beast tide strength"
    );
}

// -----------------------------------------------------------------
// P3 §8.1 #5 —— 兽潮双因子 danger 加权门槛矩阵
// -----------------------------------------------------------------

#[test]
fn beast_tide_primary_entry_danger_weight_shortens_required_duration() {
    // 三态矩阵「qi 骤降速率单独触发」态：只走主入口 `maybe_queue_beast_tide`，全程
    // 不发 PseudoVeinDissipated（次入口/塌缩因子完全缺席）。danger=7 权重(1.6)把
    // required_ticks 从 6000 缩到约 3750；用同一个 low_ticks=4200（+eval_interval 200
    // 后约 4400）验证：danger=7 应触发、danger=1（权重=1.0，仍需完整 6000）不应触发。
    let low_ticks = 4200;
    let npc_registry = NpcRegistry {
        counts_by_zone: HashMap::from([("scorch".to_string(), 6), ("spawn".to_string(), 6)]),
        ..Default::default()
    };

    let mut heartbeat_high_danger = WorldHeartbeat::default();
    heartbeat_high_danger
        .low_qi_ticks_by_zone
        .insert("scorch".to_string(), low_ticks);
    let zones_high_danger = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![zone_with_danger("scorch", 0.0, 0.0, 0.05, 7)],
    };
    maybe_queue_beast_tide(
        &mut heartbeat_high_danger,
        &zones_high_danger,
        Some(&npc_registry),
        &ActiveEventsResource::default(),
        rhythm_context(PlayerLoopPhase::DeepGathering, 100_000),
        None,
    );
    assert_eq!(
        heartbeat_high_danger.pending_omens.len(),
        1,
        "danger=7 的 zone 在 low_ticks={low_ticks} 时应已越过缩短后的有效阈值\
         （约 3750，权重 1.6）触发兽潮预警，实际未触发——danger 加权可能没接上主入口"
    );

    let mut heartbeat_low_danger = WorldHeartbeat::default();
    heartbeat_low_danger
        .low_qi_ticks_by_zone
        .insert("spawn".to_string(), low_ticks);
    let zones_low_danger = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![zone_with_danger("spawn", 0.0, 0.0, 0.05, 1)],
    };
    maybe_queue_beast_tide(
        &mut heartbeat_low_danger,
        &zones_low_danger,
        Some(&npc_registry),
        &ActiveEventsResource::default(),
        rhythm_context(PlayerLoopPhase::DeepGathering, 100_000),
        None,
    );
    assert!(
        heartbeat_low_danger.pending_omens.is_empty(),
        "danger=1 权重=1.0，同样 low_ticks={low_ticks} 未达完整 6000 阈值不应触发——\
         若触发说明 danger 加权错误地放宽了低危 zone 的门槛"
    );
}

#[test]
fn beast_tide_primary_entry_danger_weight_scales_intensity() {
    // danger 权重同时放大兽潮强度：同样的 npc_count，danger=7 队列出的 intensity
    // 应严格高于 danger=1（两者都走默认强度公式，未设 override）。
    let npc_registry = NpcRegistry {
        counts_by_zone: HashMap::from([("scorch".to_string(), 6), ("spawn".to_string(), 6)]),
        ..Default::default()
    };

    let mut heartbeat_high = WorldHeartbeat::default();
    heartbeat_high
        .low_qi_ticks_by_zone
        .insert("scorch".to_string(), BEAST_TIDE_LOW_QI_REQUIRED_TICKS);
    maybe_queue_beast_tide(
        &mut heartbeat_high,
        &ZoneRegistry {
            spatial_revision: 0,
            zones: vec![zone_with_danger("scorch", 0.0, 0.0, 0.05, 7)],
        },
        Some(&npc_registry),
        &ActiveEventsResource::default(),
        rhythm_context(PlayerLoopPhase::DeepGathering, 100_000),
        None,
    );

    let mut heartbeat_low = WorldHeartbeat::default();
    heartbeat_low
        .low_qi_ticks_by_zone
        .insert("spawn".to_string(), BEAST_TIDE_LOW_QI_REQUIRED_TICKS);
    maybe_queue_beast_tide(
        &mut heartbeat_low,
        &ZoneRegistry {
            spatial_revision: 0,
            zones: vec![zone_with_danger("spawn", 0.0, 0.0, 0.05, 1)],
        },
        Some(&npc_registry),
        &ActiveEventsResource::default(),
        rhythm_context(PlayerLoopPhase::DeepGathering, 100_000),
        None,
    );

    assert_eq!(heartbeat_high.pending_omens.len(), 1);
    assert_eq!(heartbeat_low.pending_omens.len(), 1);
    assert!(
        heartbeat_high.pending_omens[0].intensity > heartbeat_low.pending_omens[0].intensity,
        "danger=7 intensity({}) 应严格高于 danger=1 intensity({})——danger 加权\
         应放大兽潮强度而非只影响触发时长",
        heartbeat_high.pending_omens[0].intensity,
        heartbeat_low.pending_omens[0].intensity
    );
}

#[test]
fn beast_tide_primary_entry_ignores_tsy_blueprint_zones() {
    let mut heartbeat = WorldHeartbeat::default();
    heartbeat.low_qi_ticks_by_zone.insert(
        "tsy_daneng_01_shallow".to_string(),
        BEAST_TIDE_LOW_QI_REQUIRED_TICKS,
    );
    let zones = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![tsy_zone("tsy_daneng_01_shallow", 0.0, 0.0, -0.45)],
    };
    let npc_registry = NpcRegistry {
        counts_by_zone: HashMap::from([("tsy_daneng_01_shallow".to_string(), 8)]),
        ..Default::default()
    };

    maybe_queue_beast_tide(
        &mut heartbeat,
        &zones,
        Some(&npc_registry),
        &ActiveEventsResource::default(),
        rhythm_context(PlayerLoopPhase::DeepGathering, 100_000),
        None,
    );

    assert!(
        heartbeat.pending_omens.is_empty(),
        "TSY blueprint 负灵域不能被主世界兽潮 heartbeat 排队"
    );
    assert!(
        !heartbeat
            .low_qi_ticks_by_zone
            .contains_key("tsy_daneng_01_shallow"),
        "主世界 heartbeat 应清理既有 TSY low-qi 计数，避免补载后残留状态误触发"
    );
}

#[test]
fn beast_tide_neither_factor_met_yields_no_trigger_on_either_entry() {
    // 三态矩阵第三态：primary（低灵气持续时长）与 secondary（邻域塌缩扩散）都不满足——
    // 两条入口都不应触发，即便 zone 本身 danger 很高（danger 加权只放宽门槛，不能
    // 无中生有制造触发条件）。
    let npc_registry = NpcRegistry {
        counts_by_zone: HashMap::from([("scorch".to_string(), 6)]),
        ..Default::default()
    };
    let mut heartbeat = WorldHeartbeat::default();
    heartbeat
        .low_qi_ticks_by_zone
        .insert("scorch".to_string(), 100); // 远低于 danger=7 缩放后阈值(≈3750)
    maybe_queue_beast_tide(
        &mut heartbeat,
        &ZoneRegistry {
            spatial_revision: 0,
            zones: vec![zone_with_danger("scorch", 0.0, 0.0, 0.05, 7)],
        },
        Some(&npc_registry),
        &ActiveEventsResource::default(),
        rhythm_context(PlayerLoopPhase::DeepGathering, 100_000),
        None,
    );
    assert!(
        heartbeat.pending_omens.is_empty(),
        "primary 入口 low_ticks 远未达标（100 << ~3750）不应排队兽潮预警"
    );

    let mut app = App::new();
    app.insert_resource(WorldHeartbeat::default());
    app.insert_resource(ActiveEventsResource::default());
    app.insert_resource(ZoneRegistry {
        spatial_revision: 0,
        zones: vec![
            zone("pseudo_vein_done", 0.0, 0.0, 0.0),
            zone_with_danger("healthy_neighbor", 300.0, 0.0, 0.5, 7),
        ],
    });
    app.insert_resource(NpcRegistry {
        counts_by_zone: HashMap::from([("healthy_neighbor".to_string(), 4)]),
        ..Default::default()
    });
    app.add_event::<EventChainTrigger>();
    app.add_systems(Update, chain_reaction_tick);
    app.world_mut()
        .send_event(EventChainTrigger::PseudoVeinDissipated {
            zone_name: "pseudo_vein_done".to_string(),
            redistributed_qi: 0.7,
        });
    app.update();
    let active = app.world().resource::<ActiveEventsResource>();
    assert!(
        !active.contains("healthy_neighbor", EVENT_BEAST_TIDE),
        "邻域 spirit_qi=0.5 远高于任何 danger 加权后的阈值上限(0.15*1.6=0.24)，\
         次入口也不该触发——两条入口都不满足才是正确的第三态"
    );
}

#[test]
fn world_pressure_ignores_tsy_blueprint_zones() {
    let mut heartbeat = WorldHeartbeat::default();
    let zones = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![
            zone("spawn", 0.0, 0.0, 0.8),
            zone("waste", 300.0, 0.0, 0.2),
            tsy_zone("tsy_daneng_01_deep", 600.0, 0.0, -0.95),
        ],
    };

    let pressure = compute_world_pressure(
        &mut heartbeat,
        &zones,
        &[
            PlayerSample {
                player_id: "home".to_string(),
                dimension: DimensionKind::Overworld,
                zone_name: Some("spawn".to_string()),
                position: DVec3::ZERO,
                high_realm: false,
            },
            PlayerSample {
                player_id: "tsy_high".to_string(),
                dimension: DimensionKind::Tsy,
                zone_name: Some("tsy_daneng_01_deep".to_string()),
                position: DVec3::ZERO,
                high_realm: true,
            },
        ],
        1_000,
    );

    assert!(
        (pressure.avg_zone_qi - 0.5).abs() < 1e-9,
        "主世界 heartbeat pressure 只能统计 Overworld zone；TSY 负灵气不应把平均值拖到 {}",
        pressure.avg_zone_qi
    );
    assert_eq!(
        pressure.player_density_peak, 1.0,
        "TSY 玩家样本不能计入主世界 heartbeat 玩家密度"
    );
    assert_eq!(
        pressure.high_realm_count, 0,
        "TSY 高境界玩家不能抬高主世界 heartbeat high_realm_count"
    );
}

#[test]
fn heartbeat_loop_phase_uses_zone_risk_without_new_player_state() {
    let zones = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![
            zone(DEFAULT_SPAWN_ZONE_NAME, 0.0, 0.0, 0.9),
            zone("route_ash", 200.0, 0.0, 0.05),
            zone("deep_gift", 500.0, 0.0, 0.7),
        ],
    };

    assert_eq!(
        heartbeat_loop_phase(&zones, &[]),
        PlayerLoopPhase::SafeShelter
    );
    assert_eq!(
        heartbeat_loop_phase(
            &zones,
            &[PlayerSample {
                player_id: "home".to_string(),
                dimension: DimensionKind::Overworld,
                zone_name: Some(DEFAULT_SPAWN_ZONE_NAME.to_string()),
                position: DVec3::ZERO,
                high_realm: false,
            }]
        ),
        PlayerLoopPhase::HomeOrganizing
    );
    assert_eq!(
        heartbeat_loop_phase(
            &zones,
            &[PlayerSample {
                player_id: "deep".to_string(),
                dimension: DimensionKind::Overworld,
                zone_name: Some("deep_gift".to_string()),
                position: DVec3::ZERO,
                high_realm: false,
            }]
        ),
        PlayerLoopPhase::DeepGathering
    );
    assert_eq!(
        heartbeat_loop_phase(
            &zones,
            &[PlayerSample {
                player_id: "return".to_string(),
                dimension: DimensionKind::Overworld,
                zone_name: Some("route_ash".to_string()),
                position: DVec3::ZERO,
                high_realm: false,
            }]
        ),
        PlayerLoopPhase::ReturnTrip
    );
}

#[test]
fn heartbeat_loop_phase_ignores_tsy_player_samples() {
    let mut tsy_deep = tsy_zone("tsy_daneng_01_deep", 0.0, 0.0, -0.95);
    tsy_deep.danger_level = DEEP_GATHERING_DANGER_LEVEL;
    let zones = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![zone(DEFAULT_SPAWN_ZONE_NAME, 200.0, 0.0, 0.5), tsy_deep],
    };

    assert_eq!(
        heartbeat_loop_phase(
            &zones,
            &[PlayerSample {
                player_id: "tsy_high".to_string(),
                dimension: DimensionKind::Tsy,
                zone_name: Some("tsy_daneng_01_deep".to_string()),
                position: DVec3::ZERO,
                high_realm: true,
            }]
        ),
        PlayerLoopPhase::SafeShelter,
        "TSY 深层玩家不能把主世界 heartbeat 节奏推成 DeepGathering/ReturnTrip"
    );
}

#[test]
fn rhythm_table_changes_heartbeat_omen_lead_by_loop_phase() {
    let pseudo_return = rhythm_omen_lead_ticks(
        HeartbeatEventKind::PseudoVein,
        PlayerLoopPhase::ReturnTrip,
        PSEUDO_VEIN_OMEN_LEAD_TICKS,
    );
    let pseudo_deep = rhythm_omen_lead_ticks(
        HeartbeatEventKind::PseudoVein,
        PlayerLoopPhase::DeepGathering,
        PSEUDO_VEIN_OMEN_LEAD_TICKS,
    );
    let beast_deep = rhythm_omen_lead_ticks(
        HeartbeatEventKind::BeastTide,
        PlayerLoopPhase::DeepGathering,
        BEAST_TIDE_OMEN_LEAD_TICKS,
    );
    let pseudo_deep_cadence = rhythm_cadence_multiplier(
        HeartbeatEventKind::PseudoVein,
        PlayerLoopPhase::DeepGathering,
    );
    let baseline_interval =
        EventCadence::new(PSEUDO_VEIN_OMEN_LEAD_TICKS).effective_interval_ticks(1.0);
    let downfrequency_interval = EventCadence::new(PSEUDO_VEIN_OMEN_LEAD_TICKS)
        .effective_interval_ticks(pseudo_deep_cadence);

    assert!(
        pseudo_return < pseudo_deep,
        "伪灵脉应在回程阶段更快显形：return={pseudo_return} deep={pseudo_deep}"
    );
    assert!(
        beast_deep < BEAST_TIDE_OMEN_LEAD_TICKS,
        "兽潮在深处采集阶段应缩短预警窗口，形成当趟撤离压力"
    );
    assert!(
        pseudo_deep_cadence < 1.0 && downfrequency_interval > baseline_interval,
        "频率倍率小于 1 时应拉长事件间隔：multiplier={pseudo_deep_cadence} baseline={baseline_interval} downfrequency={downfrequency_interval}"
    );
}

#[test]
fn tide_sky_omen_consumes_xizhuan_boundary_and_rhythm_timing() {
    let mut heartbeat = WorldHeartbeat::default();
    let zones = ZoneRegistry::fallback();
    let boundary_tick = TICKS_PER_HOUR;
    let current_tick = boundary_tick + HEARTBEAT_EVAL_INTERVAL_TICKS;

    maybe_queue_tide_sky_omen(
        &mut heartbeat,
        &zones,
        Season::SummerToWinter,
        Some(boundary_tick),
        rhythm_context(PlayerLoopPhase::HomeOrganizing, current_tick),
        None,
    );

    assert_eq!(heartbeat.pending_omens.len(), 1);
    assert_eq!(heartbeat.pending_omens[0].kind, OmenKind::TideSkyTurning);
    assert_eq!(
        heartbeat.pending_omens[0].zone_name,
        DEFAULT_SPAWN_ZONE_NAME
    );
    assert_eq!(
        heartbeat.pending_omens[0].fires_at_tick,
        current_tick
            + rhythm_omen_lead_ticks(
                HeartbeatEventKind::TideSkyOmen,
                PlayerLoopPhase::HomeOrganizing,
                TIDE_SKY_OMEN_LEAD_TICKS,
            ),
        "汐转天象应使用 event_rhythm.json 的 home_organizing lead_ticks"
    );

    maybe_queue_tide_sky_omen(
        &mut heartbeat,
        &zones,
        Season::SummerToWinter,
        Some(boundary_tick),
        rhythm_context(PlayerLoopPhase::HomeOrganizing, current_tick + 1),
        None,
    );
    assert_eq!(
        heartbeat.pending_omens.len(),
        1,
        "同一个汐转边界只能刷新一次天象预兆，不能每个 heartbeat 重复刷"
    );
}

#[test]
fn heartbeat_tick_fires_tide_sky_omen_into_recent_events() {
    let mut app = App::new();
    let mut season_state = WorldSeasonState::default();
    let boundary_tick = TICKS_PER_HOUR;
    season_state.set_phase(Season::SummerToWinter, boundary_tick);

    app.insert_resource(WorldHeartbeat::default());
    app.insert_resource(CultivationClock {
        tick: boundary_tick + HEARTBEAT_EVAL_INTERVAL_TICKS,
    });
    app.insert_resource(season_state);
    app.insert_resource(ActiveEventsResource::default());
    app.insert_resource(ZoneRegistry::fallback());
    app.insert_resource(WorldQiAccount::default());
    app.add_event::<EventChainTrigger>();
    app.add_event::<QiTransfer>();
    app.add_systems(Update, heartbeat_tick);
    app.update();

    let heartbeat = app.world().resource::<WorldHeartbeat>();
    assert!(
        !heartbeat.pending_omens.is_empty(),
        "expected at least one pending omen because xizhuan boundary should queue tide sky omen, actual pending_omens.len()={}",
        heartbeat.pending_omens.len()
    );
    let fires_at_tick = heartbeat.pending_omens[0].fires_at_tick;
    app.world_mut().resource_mut::<CultivationClock>().tick = fires_at_tick;
    app.update();

    let recent = app
        .world()
        .resource::<ActiveEventsResource>()
        .recent_events_snapshot();
    assert!(
        recent.iter().any(|event| {
            event.target.as_deref() == Some("tide_sky_omen")
                && event.zone.as_deref() == Some(DEFAULT_SPAWN_ZONE_NAME)
        }),
        "汐转期天象不应只停留在 JSON 声明，应由 heartbeat 触发为运行时 recent event"
    );
    assert_eq!(
        app.world()
            .resource::<WorldHeartbeat>()
            .event_counts
            .get(&HeartbeatEventKind::TideSkyOmen)
            .copied(),
        Some(1),
        "汐转期天象触发后应记录 heartbeat 事件计数，证明运行时消费成功"
    );
}

#[test]
fn realm_collapse_queues_only_when_collapsing_zone_is_empty() {
    let zones = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![zone("dead_zone", 0.0, 0.0, 0.0)],
    };
    let mut heartbeat = WorldHeartbeat::default();
    heartbeat.dead_qi_ticks_by_zone.insert(
        "dead_zone".to_string(),
        REALM_COLLAPSE_DEAD_QI_REQUIRED_TICKS,
    );

    maybe_queue_realm_collapse(
        &mut heartbeat,
        &zones,
        &[],
        &ActiveEventsResource::default(),
        rhythm_context(PlayerLoopPhase::SafeShelter, TICKS_PER_HOUR),
        None,
    );

    assert_eq!(
        heartbeat.pending_omens.len(),
        1,
        "无人停留的死域应排队域崩预兆，让安全区玩家从远处感知"
    );
    assert_eq!(
        heartbeat.pending_omens[0].kind,
        OmenKind::RealmCollapseImminent
    );

    let mut occupied = WorldHeartbeat::default();
    occupied.dead_qi_ticks_by_zone.insert(
        "dead_zone".to_string(),
        REALM_COLLAPSE_DEAD_QI_REQUIRED_TICKS,
    );
    maybe_queue_realm_collapse(
        &mut occupied,
        &zones,
        &[PlayerSample {
            player_id: "stranded".to_string(),
            dimension: DimensionKind::Overworld,
            zone_name: Some("dead_zone".to_string()),
            position: DVec3::ZERO,
            high_realm: false,
        }],
        &ActiveEventsResource::default(),
        rhythm_context(PlayerLoopPhase::DeepGathering, TICKS_PER_HOUR),
        None,
    );

    assert!(
        occupied.pending_omens.is_empty(),
        "有修士停留时不应按 P4 的无人停留域崩时机排队"
    );
}

#[test]
fn realm_collapse_heartbeat_ignores_tsy_blueprint_zones() {
    let zones = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![tsy_zone("tsy_daneng_01_deep", 0.0, 0.0, 0.0)],
    };
    let mut heartbeat = WorldHeartbeat::default();
    heartbeat.dead_qi_ticks_by_zone.insert(
        "tsy_daneng_01_deep".to_string(),
        REALM_COLLAPSE_DEAD_QI_REQUIRED_TICKS,
    );

    maybe_queue_realm_collapse(
        &mut heartbeat,
        &zones,
        &[],
        &ActiveEventsResource::default(),
        rhythm_context(PlayerLoopPhase::DeepGathering, TICKS_PER_HOUR),
        None,
    );

    assert!(
        heartbeat.pending_omens.is_empty(),
        "TSY blueprint zone 不能被主世界无人域崩 heartbeat 排队"
    );
    assert!(
        !heartbeat
            .dead_qi_ticks_by_zone
            .contains_key("tsy_daneng_01_deep"),
        "主世界 heartbeat 应清理既有 TSY dead-qi 计数"
    );
}

#[test]
fn force_override_replaces_existing_pending_omen() {
    let mut heartbeat = WorldHeartbeat::default();
    heartbeat.pending_omens.push(WorldEventOmen {
        kind: OmenKind::BeastTideApproaching,
        zone_name: "hungry".to_string(),
        target_player: None,
        origin: DVec3::new(0.0, 65.0, 0.0),
        intensity: 0.1,
        scheduled_at_tick: 0,
        fires_at_tick: 10_000,
        expires_at_tick: 10_200,
    });
    heartbeat.forced_events.push(ForcedHeartbeatEvent {
        event_kind: HeartbeatEventKind::BeastTide,
        target_zone: "hungry".to_string(),
        intensity: 0.9,
    });
    let zones = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![zone("hungry", 0.0, 0.0, 0.1)],
    };

    queue_forced_events(&mut heartbeat, &zones, 200, None);

    assert_eq!(heartbeat.pending_omens.len(), 1);
    assert_eq!(
        heartbeat.pending_omens[0].intensity, 0.9,
        "force override should replace the older same-zone pending omen"
    );
    assert_eq!(
        heartbeat.pending_omens[0].fires_at_tick, 200,
        "force override should fire at the current heartbeat tick"
    );
}

#[test]
fn real_heartbeat_system_force_override_fires_through_app() {
    let mut heartbeat = WorldHeartbeat::default();
    heartbeat.apply_override(
        HeartbeatOverrideAction::Force,
        HeartbeatEventKind::BeastTide,
        "spawn".to_string(),
        100,
        Some(0.8),
        0,
    );

    let mut app = App::new();
    app.insert_resource(heartbeat);
    app.insert_resource(CultivationClock {
        tick: HEARTBEAT_EVAL_INTERVAL_TICKS,
    });
    app.insert_resource(ActiveEventsResource::default());
    app.insert_resource(ZoneRegistry::fallback());
    app.insert_resource(WorldQiAccount::default());
    app.add_event::<EventChainTrigger>();
    app.add_event::<QiTransfer>();
    app.add_systems(
        Update,
        (heartbeat_tick, chain_reaction_tick.after(heartbeat_tick)),
    );
    app.update();

    let active = app.world().resource::<ActiveEventsResource>();
    assert!(
        active.contains("spawn", EVENT_BEAST_TIDE),
        "real heartbeat_tick system should fire a forced beast tide through ActiveEventsResource"
    );
    let heartbeat = app.world().resource::<WorldHeartbeat>();
    assert_eq!(
        heartbeat
            .event_counts
            .get(&HeartbeatEventKind::BeastTide)
            .copied(),
        Some(1),
        "real heartbeat_tick path should record the fired beast tide"
    );
}

#[test]
fn simulated_48h_unattended_world_meets_plan_floor() {
    let report = simulate_unattended_world(48, 10);

    assert!(report.pseudo_vein_count >= 80);
    assert!(report.beast_tide_count >= 30);
    assert!(report.realm_collapse_count >= 5);
    assert!(report.karma_backlash_count >= 40);
    assert!(report.chain_reaction_count >= 10);
    assert!(report.qi_total_delta_ratio < 0.05);
    assert!(report.max_same_zone_stack <= 3);
}

// ───────────────────── plan-zone-qi-economy-v1 P1 — zone_qi_inflow_tick ─────────────────────

fn inflow_test_app(zones: Vec<Zone>, pending_pool_balance: f64, start_tick: u64) -> App {
    let mut app = App::new();
    app.insert_resource(ZoneQiInflowClock::default());
    app.insert_resource(CultivationClock { tick: start_tick });
    app.insert_resource(ZoneRegistry {
        zones,
        spatial_revision: 0,
    });
    app.insert_resource(ActiveEventsResource::default());
    let mut ledger = WorldQiAccount::default();
    if pending_pool_balance > 0.0 {
        ledger
            .set_balance(pending_inflow_account(), pending_pool_balance)
            .expect("seeding the pending pool balance must succeed");
    }
    app.insert_resource(ledger);
    app.add_systems(Update, zone_qi_inflow_tick);
    app
}

fn advance_ticks(app: &mut App, ticks: u64) {
    let mut clock = app.world_mut().resource_mut::<CultivationClock>();
    clock.tick = clock.tick.saturating_add(ticks);
    app.update();
}

#[test]
fn zero_elapsed_ticks_on_first_run_is_a_noop() {
    // 首次 run：ZoneQiInflowClock::default() 的 last_tick=0，若 CultivationClock 也从 0
    // 起步，elapsed_ticks==0，不应该做任何事（也不应该 panic）。
    let mut z = zone("spawn", 0.0, 0.0, 0.1);
    z.qi_equilibrium = 0.5;
    z.qi_inflow_per_min = 1.0;
    let mut app = inflow_test_app(vec![z], 1000.0, 0);
    app.update();

    let zones = app.world().resource::<ZoneRegistry>();
    assert_eq!(
        zones.zones[0].spirit_qi, 0.1,
        "tick delta of zero (both clocks start at 0) must not inject any qi"
    );
}

#[test]
fn injects_from_pending_pool_and_debits_it_by_the_same_amount() {
    let mut z = zone("spawn", 0.0, 0.0, 0.1);
    z.qi_equilibrium = 0.5;
    z.qi_inflow_per_min = 1.0; // 1.0 绝对点/分钟
    let mut app = inflow_test_app(vec![z], 1000.0, 0);

    // 1 分钟 = TICKS_PER_MINUTE ticks
    advance_ticks(&mut app, TICKS_PER_MINUTE);

    let zones = app.world().resource::<ZoneRegistry>();
    let expected_fraction_gain = 1.0 / QI_ZONE_UNIT_CAPACITY; // 1.0 absolute / 50.0 capacity
    assert!(
        (zones.zones[0].spirit_qi - (0.1 + expected_fraction_gain)).abs() < 1e-9,
        "after 1 minute at 1.0/min, spirit_qi should rise by 1.0/QI_ZONE_UNIT_CAPACITY \
         ({expected_fraction_gain}), got {}",
        zones.zones[0].spirit_qi
    );

    let ledger = app.world().resource::<WorldQiAccount>();
    assert!(
        (ledger.balance(&pending_inflow_account()) - (1000.0 - 1.0)).abs() < 1e-9,
        "pending pool must be debited by exactly the absolute amount credited to the zone \
         (conservation: pool loses 1.0, zone gains 1.0/CAPACITY fraction == 1.0 absolute), \
         got pool balance {}",
        ledger.balance(&pending_inflow_account())
    );
}

#[test]
fn clamps_at_equilibrium_and_never_overshoots_across_many_ticks() {
    let mut z = zone("spawn", 0.0, 0.0, 0.3);
    z.qi_equilibrium = 0.35;
    z.qi_inflow_per_min = 5.0; // deliberately fast so it would overshoot without the clamp
    let mut app = inflow_test_app(vec![z], 100_000.0, 0);

    // Run many minutes' worth of ticks — should settle at equilibrium and stop.
    advance_ticks(&mut app, TICKS_PER_MINUTE * 50);

    let zones = app.world().resource::<ZoneRegistry>();
    assert!(
        zones.zones[0].spirit_qi <= 0.35 + 1e-9,
        "spirit_qi ({}) must never exceed qi_equilibrium (0.35), even after many ticks of \
         a fast inflow rate that would overshoot without clamping",
        zones.zones[0].spirit_qi
    );
    assert!(
        zones.zones[0].spirit_qi >= 0.35 - 1e-6,
        "spirit_qi ({}) should have converged to equilibrium (0.35) given ample pool and \
         many ticks",
        zones.zones[0].spirit_qi
    );

    // Run further — must remain pinned, not creep past equilibrium.
    advance_ticks(&mut app, TICKS_PER_MINUTE * 50);
    let zones = app.world().resource::<ZoneRegistry>();
    assert!(
        zones.zones[0].spirit_qi <= 0.35 + 1e-9,
        "continuing to tick after reaching equilibrium must not push spirit_qi past it \
         (got {})",
        zones.zones[0].spirit_qi
    );
}

#[test]
fn insufficient_pending_pool_scales_down_and_never_overdraws() {
    let mut z = zone("spawn", 0.0, 0.0, 0.1);
    z.qi_equilibrium = 0.9;
    z.qi_inflow_per_min = 10.0;
    // Pool only has 2.0 absolute points — far less than what 1 minute at 10.0/min would need.
    let mut app = inflow_test_app(vec![z], 2.0, 0);

    advance_ticks(&mut app, TICKS_PER_MINUTE);

    let ledger = app.world().resource::<WorldQiAccount>();
    let pool_balance = ledger.balance(&pending_inflow_account());
    assert!(
        pool_balance >= -1e-9,
        "pending pool balance must never go negative (no overdraw), got {pool_balance}"
    );
    assert!(
        pool_balance.abs() < 1e-9,
        "with only 2.0 available and 10.0 desired, the pool should be drained to exactly \
         zero (scaled down), not partially retained or overdrawn — got {pool_balance}"
    );

    let zones = app.world().resource::<ZoneRegistry>();
    let expected_fraction_gain = 2.0 / QI_ZONE_UNIT_CAPACITY;
    assert!(
        (zones.zones[0].spirit_qi - (0.1 + expected_fraction_gain)).abs() < 1e-9,
        "the zone must only receive the amount the pool could actually afford (2.0 \
         absolute -> {expected_fraction_gain} fraction), got {}",
        zones.zones[0].spirit_qi
    );
}

#[test]
fn empty_pending_pool_yields_zero_inflow_and_zone_is_untouched() {
    let mut z = zone("spawn", 0.0, 0.0, 0.1);
    z.qi_equilibrium = 0.5;
    z.qi_inflow_per_min = 1.0;
    let mut app = inflow_test_app(vec![z], 0.0, 0);

    advance_ticks(&mut app, TICKS_PER_MINUTE * 10);

    let zones = app.world().resource::<ZoneRegistry>();
    assert_eq!(
        zones.zones[0].spirit_qi, 0.1,
        "an empty pending pool must leave the zone completely untouched, not partially \
         credit it or panic"
    );
}

#[test]
fn negative_zone_qi_is_skipped_entirely() {
    let mut z = zone("dead_zone", 0.0, 0.0, -0.2);
    z.qi_equilibrium = 0.5;
    z.qi_inflow_per_min = 1.0;
    let mut app = inflow_test_app(vec![z], 1000.0, 0);

    advance_ticks(&mut app, TICKS_PER_MINUTE * 10);

    let zones = app.world().resource::<ZoneRegistry>();
    assert_eq!(
        zones.zones[0].spirit_qi, -0.2,
        "negative-qi (负灵域) zones must never be inflowed by this P1 system — recovery \
         out of negative territory is explicitly out of scope (§8.1 #5)"
    );
    let ledger = app.world().resource::<WorldQiAccount>();
    assert_eq!(
        ledger.balance(&pending_inflow_account()),
        1000.0,
        "the pending pool must not be touched at all for a skipped negative-qi zone"
    );
}

#[test]
fn realm_collapse_zone_is_skipped_even_when_below_equilibrium() {
    let mut z = zone("collapsing", 0.0, 0.0, 0.1);
    z.qi_equilibrium = 0.5;
    z.qi_inflow_per_min = 1.0;
    let mut app = inflow_test_app(vec![z], 1000.0, 0);
    {
        let command = spawn_event_command("collapsing", EVENT_REALM_COLLAPSE, 1.0, 20_000, None);
        let mut zones_for_lookup = app.world().resource::<ZoneRegistry>().clone();
        let mut active_events = app.world_mut().resource_mut::<ActiveEventsResource>();
        assert!(
            active_events.enqueue_from_spawn_command(&command, Some(&mut zones_for_lookup)),
            "test setup: enqueueing the REALM_COLLAPSE active event must succeed"
        );
    }

    advance_ticks(&mut app, TICKS_PER_MINUTE * 10);

    let zones = app.world().resource::<ZoneRegistry>();
    assert_eq!(
        zones.zones[0].spirit_qi, 0.1,
        "a zone with an active EVENT_REALM_COLLAPSE must be skipped by inflow even though \
         it is far below equilibrium (§8.1 #5, mirrors maybe_queue_realm_collapse's own \
         active_events.contains(..., EVENT_REALM_COLLAPSE) gate)"
    );
}

#[test]
fn zero_equilibrium_zone_is_never_touched_back_compat() {
    // 默认值 0.0（没配置 qi_equilibrium/qi_inflow_per_min 的旧 zone）必须完全不受影响。
    let z = zone("legacy_zone", 0.0, 0.0, 0.05);
    assert_eq!(z.qi_equilibrium, 0.0);
    assert_eq!(z.qi_inflow_per_min, 0.0);
    let mut app = inflow_test_app(vec![z], 1000.0, 0);

    advance_ticks(&mut app, TICKS_PER_MINUTE * 100);

    let zones = app.world().resource::<ZoneRegistry>();
    assert_eq!(
        zones.zones[0].spirit_qi, 0.05,
        "a zone with qi_equilibrium == 0.0 (back-compat default) must never receive any \
         inflow, no matter how many ticks pass"
    );
    let ledger = app.world().resource::<WorldQiAccount>();
    assert_eq!(
        ledger.balance(&pending_inflow_account()),
        1000.0,
        "the pending pool must be completely untouched for an opted-out zone"
    );
}

#[test]
fn multi_zone_conservation_holds_across_a_long_run() {
    // 回流↔（模拟）吸收长跑总量守恒：多个 zone 分别从同一个待分配池取用，
    // 待分配池减少量之和必须精确等于所有 zone 累计增加量之和（换算到绝对单位）。
    let mut zone_a = zone("zone_a", 0.0, 0.0, 0.05);
    zone_a.qi_equilibrium = 0.3;
    zone_a.qi_inflow_per_min = 0.6;
    let mut zone_b = zone("zone_b", 500.0, 0.0, 0.1);
    zone_b.qi_equilibrium = 0.4;
    zone_b.qi_inflow_per_min = 0.3;
    let mut zone_c_no_inflow = zone("zone_c", 1000.0, 0.0, 0.05);
    zone_c_no_inflow.qi_equilibrium = 0.0; // opted out, must stay untouched

    let initial_pool = 500.0;
    let mut app = inflow_test_app(vec![zone_a, zone_b, zone_c_no_inflow], initial_pool, 0);

    for _ in 0..200 {
        advance_ticks(&mut app, TICKS_PER_MINUTE);
    }

    let zones = app.world().resource::<ZoneRegistry>();
    let ledger = app.world().resource::<WorldQiAccount>();
    let pool_balance = ledger.balance(&pending_inflow_account());

    let zone_a_absolute = zones.zones[0].spirit_qi * QI_ZONE_UNIT_CAPACITY;
    let zone_b_absolute = zones.zones[1].spirit_qi * QI_ZONE_UNIT_CAPACITY;
    let zone_a_initial_absolute = 0.05 * QI_ZONE_UNIT_CAPACITY;
    let zone_b_initial_absolute = 0.1 * QI_ZONE_UNIT_CAPACITY;
    let total_credited =
        (zone_a_absolute - zone_a_initial_absolute) + (zone_b_absolute - zone_b_initial_absolute);
    let total_debited = initial_pool - pool_balance;

    assert!(
        (total_credited - total_debited).abs() < 1e-6,
        "sum of absolute qi credited to all zones ({total_credited}) must exactly equal \
         the amount debited from the shared pending pool ({total_debited}) — any mismatch \
         is qi being created or destroyed out of thin air"
    );
    assert_eq!(
        zones.zones[2].spirit_qi, 0.05,
        "the opted-out zone_c (qi_equilibrium == 0.0) must never participate and must \
         stay completely untouched even while its siblings draw from the shared pool"
    );
    assert!(
        zones.zones[0].spirit_qi <= 0.3 + 1e-9 && zones.zones[1].spirit_qi <= 0.4 + 1e-9,
        "neither zone may overshoot its own equilibrium after a long multi-zone run \
         (zone_a={}, zone_b={})",
        zones.zones[0].spirit_qi,
        zones.zones[1].spirit_qi
    );
}

#[test]
fn time_advance_style_large_tick_jump_is_caught_up_in_one_evaluation() {
    // `/time advance` 直接 saturating_add 到 CultivationClock.tick（不是逐 tick 递增），
    // 下一次 zone_qi_inflow_tick 必须按整段 delta 一次性补上，而不是只补 1 tick 的量。
    let mut z = zone("spawn", 0.0, 0.0, 0.1);
    z.qi_equilibrium = 0.9;
    z.qi_inflow_per_min = 1.0;
    let mut app = inflow_test_app(vec![z], 100_000.0, 0);

    // Jump 30 minutes' worth of ticks all at once, like `/time advance` would.
    advance_ticks(&mut app, TICKS_PER_MINUTE * 30);

    let zones = app.world().resource::<ZoneRegistry>();
    let expected_fraction_gain = (1.0 * 30.0) / QI_ZONE_UNIT_CAPACITY;
    assert!(
        (zones.zones[0].spirit_qi - (0.1 + expected_fraction_gain)).abs() < 1e-9,
        "a single large tick jump (simulating /time advance) must be caught up as one \
         30-minute window (gain={expected_fraction_gain}), not truncated to a single \
         per-tick increment — got {}",
        zones.zones[0].spirit_qi
    );
}
