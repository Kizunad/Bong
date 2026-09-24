use std::collections::HashMap;

use bong_server::cultivation::components::Cultivation;
use bong_server::cultivation::tick::CultivationClock;
use bong_server::network::vfx_event_emit::VfxEventRequest;
use bong_server::player::gameplay::PendingGameplayNarrations;
use bong_server::qi_physics::constants::QI_ZONE_UNIT_CAPACITY;
use bong_server::qi_physics::ledger::{
    assert_conservation, pending_inflow_account, transfer_zone_qi_to_ledger, QiAccountId,
    QiTransfer, QiTransferReason, WorldQiAccount, WorldQiSnapshot,
};
use bong_server::schema::common::{NarrationScope, NarrationStyle, SPIRIT_QI_TOTAL};
use bong_server::schema::pseudo_vein::PseudoVeinSeasonV1;
use bong_server::schema::vfx_event::VfxEventPayloadV1;
use bong_server::world::dimension::DimensionKind;
use bong_server::world::pseudo_vein_runtime::fallback_auto_spawn_on_high_drain as production_fallback_auto_spawn_on_high_drain;
use bong_server::world::pseudo_vein_runtime::*;
use bong_server::world::zone::{Zone, ZoneRegistry};
use valence::prelude::{App, BlockPos, DVec3, Events, Position, Update};

#[test]
fn rising_reaches_max_qi_in_600_ticks() {
    let mut runtime = runtime(PseudoVeinSeasonV1::Summer);

    let midway = runtime.advance(300, 0);
    assert_eq!(midway.current_qi, PSEUDO_VEIN_MAX_QI / 2.0);
    assert_eq!(midway.phase, PseudoVeinPhase::Rising);

    let risen = runtime.advance(600, 0);
    assert_eq!(risen.current_qi, PSEUDO_VEIN_MAX_QI);
    assert_eq!(risen.phase, PseudoVeinPhase::Active);
}

#[test]
fn crowded_dissipates_faster() {
    let mut quiet = runtime(PseudoVeinSeasonV1::Summer);
    let mut crowded = runtime(PseudoVeinSeasonV1::Summer);
    quiet.advance(600, 0);
    crowded.advance(600, 0);

    let quiet_after = quiet.advance(600 + 6_000, 1);
    let crowded_after = crowded.advance(600 + 6_000, 5);

    assert!(
        crowded_after.current_qi < quiet_after.current_qi,
        "5 人聚集应比 1 人更快消耗伪灵脉"
    );
    assert_eq!(pseudo_vein_decay_multiplier(5), 3.5);
}

#[test]
fn qi_conservation() {
    // plan-zone-qi-economy-v1 P3 §8.1 决议 #3 — 借多少还多少（100% 归还），不是旧版本
    // 固定 30% 比例（那条路 70% 会永久留在 zone，凭空创生）。
    let settlement = settle_pseudo_vein_qi("lingquan_marsh", 37.5);

    assert_eq!(
        settlement.returned_to_pool, settlement.injected_qi,
        "settlement must plan to return exactly what was borrowed (100%), not a partial \
         ratio — got injected_qi={} returned_to_pool={}",
        settlement.injected_qi, settlement.returned_to_pool
    );
    assert_eq!(
        settlement.return_transfer.from,
        QiAccountId::zone("lingquan_marsh")
    );
    assert_eq!(settlement.return_transfer.to, pending_inflow_account());
    assert_eq!(settlement.return_transfer.amount, settlement.injected_qi);
    assert_eq!(
        settlement.return_transfer.reason,
        QiTransferReason::PseudoVeinSettle
    );

    // The pure settlement plan above must also survive the real external-zone ledger path:
    // snapshot the complete observed total from the canonical SPIRIT_QI_TOTAL budget, execute
    // the transfer, and prove that no qi is created or lost (era decay is explicitly zero).
    let borrowed_absolute = settlement.return_transfer.amount;
    let mut zone_spirit_qi = borrowed_absolute / QI_ZONE_UNIT_CAPACITY;
    let mut ledger = WorldQiAccount::default();
    ledger
        .set_balance(
            pending_inflow_account(),
            SPIRIT_QI_TOTAL - borrowed_absolute,
        )
        .expect("the pending pool seed must be a finite non-negative budget remainder");
    let before = WorldQiSnapshot {
        player_qi: 0.0,
        zone_qi: zone_spirit_qi * QI_ZONE_UNIT_CAPACITY,
        container_qi: 0.0,
        ledger_qi: ledger.total(),
        era_decay_accum: 0.0,
        budget_initial_total: SPIRIT_QI_TOTAL,
        budget_current_total: SPIRIT_QI_TOTAL,
    };

    let applied = transfer_zone_qi_to_ledger(
        &mut ledger,
        "lingquan_marsh",
        &mut zone_spirit_qi,
        pending_inflow_account(),
        borrowed_absolute,
        QiTransferReason::PseudoVeinSettle,
    )
    .expect("the settlement transfer must be accepted by the canonical qi ledger")
    .expect("a positive settlement must emit one transfer audit");
    assert_eq!(applied, settlement.return_transfer);

    let after = WorldQiSnapshot {
        player_qi: 0.0,
        zone_qi: zone_spirit_qi * QI_ZONE_UNIT_CAPACITY,
        container_qi: 0.0,
        ledger_qi: ledger.total(),
        era_decay_accum: 0.0,
        budget_initial_total: SPIRIT_QI_TOTAL,
        budget_current_total: SPIRIT_QI_TOTAL,
    };
    assert_conservation(&before, &after, 0.0)
        .expect("pseudo-vein settlement must preserve SPIRIT_QI_TOTAL with zero era decay");
}

#[test]
fn settle_pseudo_vein_qi_rejects_negative_injected_as_zero() {
    let settlement = settle_pseudo_vein_qi("lingquan_marsh", -5.0);
    assert_eq!(
        settlement.injected_qi, 0.0,
        "a negative injected_qi (should never happen, but defensive) must clamp to a \
         zero-amount settlement, not underflow or panic"
    );
    assert_eq!(settlement.returned_to_pool, 0.0);
}

#[test]
fn aftermath_spawns_negative_hotspots() {
    let mut runtime = runtime(PseudoVeinSeasonV1::Summer);
    runtime.advance(600, 0);
    runtime.advance(600 + PSEUDO_VEIN_BASE_DURATION_TICKS, 1);

    let outcome = runtime.advance(
        600 + PSEUDO_VEIN_BASE_DURATION_TICKS + PSEUDO_VEIN_DISSIPATING_TICKS,
        1,
    );

    assert_eq!(outcome.phase, PseudoVeinPhase::StormAftermath);
    assert!(outcome.settlement.is_some());
    assert!((1..=3).contains(&outcome.aftermath_hotspots.len()));
}

#[test]
fn tide_turn_doubles_duration() {
    assert_eq!(
        effective_duration_ticks(PSEUDO_VEIN_BASE_DURATION_TICKS, PseudoVeinSeasonV1::Summer),
        PSEUDO_VEIN_BASE_DURATION_TICKS
    );
    assert_eq!(
        effective_duration_ticks(
            PSEUDO_VEIN_BASE_DURATION_TICKS,
            PseudoVeinSeasonV1::WinterToSummer,
        ),
        PSEUDO_VEIN_BASE_DURATION_TICKS * 2
    );
}

#[test]
fn fallback_auto_spawn_on_high_drain() {
    let registry = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![zone("slow", 0.4, 0.0), zone("fast", 0.2, 64.0)],
    };
    let drain = HashMap::from([("slow".to_string(), 0.01), ("fast".to_string(), 0.03)]);
    let density = HashMap::new();

    let intent = production_fallback_auto_spawn_on_high_drain(
        &registry,
        &drain,
        &density,
        PseudoVeinSeasonV1::SummerToWinter,
    )
    .expect("高消耗汐转期应触发 fallback 伪灵脉");

    assert_eq!(intent.zone_id, "fast");
    assert_eq!(intent.reason, PseudoVeinSpawnReason::TideTurnHighDrain);
    assert_eq!(intent.duration_ticks, PSEUDO_VEIN_BASE_DURATION_TICKS * 2);
}

#[test]
fn inject_zone_for_pseudo_vein_borrows_from_pending_pool_and_debits_it() {
    let mut zone = zone("fast", 0.1, 0.0);
    let mut ledger = WorldQiAccount::default();
    ledger
        .set_balance(pending_inflow_account(), 1000.0)
        .expect("seeding pool balance must succeed");

    let transfer = inject_zone_for_pseudo_vein(&mut zone, &mut ledger)
        .expect("a well-funded pending pool should fund the injection");

    let expected_absolute = round3((PSEUDO_VEIN_MAX_QI - 0.1) * QI_ZONE_UNIT_CAPACITY);
    assert_eq!(
        zone.spirit_qi, PSEUDO_VEIN_MAX_QI,
        "a well-funded pool must let the zone reach the full pseudo-vein target"
    );
    assert_eq!(transfer.from, pending_inflow_account());
    assert_eq!(transfer.to, QiAccountId::zone("fast"));
    assert_eq!(transfer.amount, expected_absolute);
    assert_eq!(transfer.reason, QiTransferReason::ReleaseToZone);
    assert_eq!(
        ledger.balance(&pending_inflow_account()),
        round3(1000.0 - expected_absolute),
        "pending pool must be debited by exactly the amount credited to the zone \
         (conservation: no qi created out of thin air)"
    );
}

#[test]
fn inject_zone_for_pseudo_vein_scales_down_when_pool_is_underfunded() {
    let mut zone = zone("fast", 0.1, 0.0);
    let mut ledger = WorldQiAccount::default();
    // Only 5.0 available — far less than the ~37.5 a full injection to PSEUDO_VEIN_MAX_QI
    // would need.
    ledger
        .set_balance(pending_inflow_account(), 5.0)
        .expect("seeding pool balance must succeed");

    let transfer = inject_zone_for_pseudo_vein(&mut zone, &mut ledger)
        .expect("a partially-funded pool should still fund a partial injection");

    assert_eq!(transfer.amount, 5.0);
    assert!(
        zone.spirit_qi < PSEUDO_VEIN_MAX_QI,
        "an underfunded pool must not let the zone reach the full pseudo-vein target \
         (would be an overdraw), got {}",
        zone.spirit_qi
    );
    assert_eq!(
        ledger.balance(&pending_inflow_account()),
        0.0,
        "the pool must be drained to exactly zero (scaled down), never left negative or \
         partially untouched"
    );
}

#[test]
fn inject_zone_for_pseudo_vein_is_a_noop_when_pool_is_empty() {
    let mut zone = zone("fast", 0.1, 0.0);
    let mut ledger = WorldQiAccount::default();

    let outcome = inject_zone_for_pseudo_vein(&mut zone, &mut ledger);

    assert!(
        outcome.is_none(),
        "an empty pending pool must yield no injection at all, not a zero-amount transfer"
    );
    assert_eq!(
        zone.spirit_qi, 0.1,
        "the zone must be left completely untouched when the pool cannot afford anything"
    );
}

#[test]
fn inject_zone_for_pseudo_vein_is_a_noop_when_zone_already_above_target() {
    let mut zone = zone("fast", PSEUDO_VEIN_MAX_QI + 0.05, 0.0);
    let mut ledger = WorldQiAccount::default();
    ledger
        .set_balance(pending_inflow_account(), 1000.0)
        .expect("seeding pool balance must succeed");

    let outcome = inject_zone_for_pseudo_vein(&mut zone, &mut ledger);

    assert!(
        outcome.is_none(),
        "a zone already above the pseudo-vein target must not receive (or drain) any qi"
    );
    assert_eq!(
        ledger.balance(&pending_inflow_account()),
        1000.0,
        "the pool must be left untouched when the zone doesn't need topping up"
    );
}

#[test]
fn runtime_tick_settlement_fully_repays_pending_pool_when_zone_still_holds_it() {
    let mut app = App::new();
    app.insert_resource(CultivationClock {
        tick: PSEUDO_VEIN_DISSIPATING_TICKS,
    });
    app.insert_resource(ZoneRegistry {
        spatial_revision: 0,
        zones: vec![zone("lingquan_marsh", PSEUDO_VEIN_MAX_QI, 0.0)],
    });
    app.insert_resource(WorldQiAccount::default());
    app.add_event::<VfxEventRequest>();
    app.add_event::<QiTransfer>();
    app.add_systems(Update, pseudo_vein_runtime_tick_system);

    let injected_absolute = round3(PSEUDO_VEIN_MAX_QI * QI_ZONE_UNIT_CAPACITY);
    let mut runtime = runtime(PseudoVeinSeasonV1::Summer);
    runtime.set_test_state(PseudoVeinPhase::Dissipating, 0, 0.0, injected_absolute, 0);
    app.world_mut().spawn(runtime);

    app.update();

    let zone_qi = app
        .world()
        .resource::<ZoneRegistry>()
        .find_zone_by_name("lingquan_marsh")
        .expect("test zone should exist")
        .spirit_qi;
    assert!(
        zone_qi.abs() < 1e-9,
        "when nobody consumed the pseudo-vein boost, settlement must return 100% of the \
         borrowed amount (not the old fixed 30%), draining the zone back to ~0, got {zone_qi}"
    );
    let transfers = app
        .world()
        .resource::<Events<QiTransfer>>()
        .iter_current_update_events()
        .collect::<Vec<_>>();
    assert_eq!(transfers.len(), 1);
    assert_eq!(transfers[0].from, QiAccountId::zone("lingquan_marsh"));
    assert_eq!(transfers[0].to, pending_inflow_account());
    assert_eq!(transfers[0].amount, injected_absolute);
    assert_eq!(transfers[0].reason, QiTransferReason::PseudoVeinSettle);
    assert_eq!(
        app.world()
            .resource::<WorldQiAccount>()
            .balance(&pending_inflow_account()),
        injected_absolute,
        "the pending pool must receive back exactly what was originally borrowed"
    );
}

#[test]
fn runtime_tick_settlement_caps_repay_at_whatever_the_zone_still_holds() {
    // 借款期间部分被玩家/NPC 正常吸收（已通过既有 regen_from_zone 路径守恒记账）——
    // 结算时只能"能还多少还多少"，不能把 zone 打成负数去凑足全额归还。
    let mut app = App::new();
    app.insert_resource(CultivationClock {
        tick: PSEUDO_VEIN_DISSIPATING_TICKS,
    });
    // zone only holds 0.2 fraction (10.0 absolute) worth of qi at settlement time, far less
    // than the 42.5 absolute that was originally borrowed.
    app.insert_resource(ZoneRegistry {
        spatial_revision: 0,
        zones: vec![zone("lingquan_marsh", 0.2, 0.0)],
    });
    app.insert_resource(WorldQiAccount::default());
    app.add_event::<VfxEventRequest>();
    app.add_event::<QiTransfer>();
    app.add_systems(Update, pseudo_vein_runtime_tick_system);

    let injected_absolute = round3(PSEUDO_VEIN_MAX_QI * QI_ZONE_UNIT_CAPACITY);
    let mut runtime = runtime(PseudoVeinSeasonV1::Summer);
    runtime.set_test_state(PseudoVeinPhase::Dissipating, 0, 0.0, injected_absolute, 0);
    app.world_mut().spawn(runtime);

    app.update();

    let zone_qi = app
        .world()
        .resource::<ZoneRegistry>()
        .find_zone_by_name("lingquan_marsh")
        .expect("test zone should exist")
        .spirit_qi;
    assert!(
        zone_qi.abs() < 1e-9,
        "the zone must be drained to exactly zero (everything it still held), not left \
         negative, got {zone_qi}"
    );
    let transfers = app
        .world()
        .resource::<Events<QiTransfer>>()
        .iter_current_update_events()
        .collect::<Vec<_>>();
    assert_eq!(transfers.len(), 1);
    assert_eq!(
        transfers[0].amount, 10.0,
        "repay must be capped at what the zone actually held (10.0 absolute), not the \
         originally-borrowed 42.5 — the difference was already legitimately consumed \
         elsewhere and cannot be conjured back"
    );
}

#[test]
fn aftermath_runtime_expires_after_ttl() {
    let mut app = App::new();
    app.insert_resource(CultivationClock {
        tick: PSEUDO_VEIN_AFTERMATH_TICKS,
    });
    app.insert_resource(ZoneRegistry {
        spatial_revision: 0,
        zones: vec![zone("lingquan_marsh", 0.42, 0.0)],
    });
    app.add_event::<VfxEventRequest>();
    app.add_event::<QiTransfer>();
    app.add_systems(Update, pseudo_vein_runtime_tick_system);

    let mut runtime = runtime(PseudoVeinSeasonV1::Summer);
    runtime.set_test_state(PseudoVeinPhase::StormAftermath, 0, 0.0, 0.0, 0);
    app.world_mut().spawn(runtime);

    app.update();

    let mut query = app.world_mut().query::<&PseudoVeinRuntime>();
    assert_eq!(query.iter(app.world()).count(), 0);
}

// ── plan-zone-qi-economy-v1 P3 §8.1 决议 #3 — 灵潮 narration pin ──

#[test]
fn phase_narration_covers_all_variants() {
    assert!(
        pseudo_vein_phase_narration(PseudoVeinPhase::Active).is_some(),
        "entering Active (窗口开启，固元门槛可用) must produce a narration cue"
    );
    assert!(
        pseudo_vein_phase_narration(PseudoVeinPhase::Dissipating).is_some(),
        "entering Dissipating (窗口关闭) must produce a narration cue"
    );
    assert_ne!(
        pseudo_vein_phase_narration(PseudoVeinPhase::Active),
        pseudo_vein_phase_narration(PseudoVeinPhase::Dissipating),
        "the two narration cues must be textually distinct, not a copy-paste placeholder"
    );
    assert!(pseudo_vein_phase_narration(PseudoVeinPhase::Rising).is_none());
    assert!(pseudo_vein_phase_narration(PseudoVeinPhase::Warning).is_none());
    assert!(pseudo_vein_phase_narration(PseudoVeinPhase::StormAftermath).is_none());
}

#[test]
fn runtime_tick_pushes_zone_scope_narration_on_entering_active() {
    let mut app = App::new();
    app.insert_resource(CultivationClock {
        tick: PSEUDO_VEIN_RISING_TICKS,
    });
    app.insert_resource(ZoneRegistry {
        spatial_revision: 0,
        zones: vec![zone("lingquan_marsh", 0.1, 0.0)],
    });
    app.insert_resource(WorldQiAccount::default());
    app.insert_resource(PendingGameplayNarrations::default());
    app.add_event::<VfxEventRequest>();
    app.add_event::<QiTransfer>();
    app.add_systems(Update, pseudo_vein_runtime_tick_system);

    let runtime = runtime(PseudoVeinSeasonV1::Summer);
    app.world_mut().spawn(runtime);

    app.update();

    let mut narrations = app
        .world_mut()
        .resource_mut::<PendingGameplayNarrations>()
        .drain();
    assert_eq!(
        narrations.len(),
        1,
        "crossing Rising -> Active must push exactly one zone-scope narration"
    );
    let narration = narrations.remove(0);
    assert_eq!(narration.scope, NarrationScope::Zone);
    assert_eq!(narration.target.as_deref(), Some("lingquan_marsh"));
    assert_eq!(narration.style, NarrationStyle::Perception);
    assert_eq!(
        narration.text,
        pseudo_vein_phase_narration(PseudoVeinPhase::Active).unwrap()
    );
}

#[test]
fn fallback_system_spawns_runtime_on_high_density() {
    let mut app = App::new();
    app.insert_resource(CultivationClock {
        tick: PSEUDO_VEIN_FALLBACK_EVAL_PERIOD_TICKS,
    });
    app.insert_resource(ZoneRegistry {
        spatial_revision: 0,
        zones: vec![zone("fast", 0.1, 0.0)],
    });
    app.insert_resource(PseudoVeinFallbackState::from_test_snapshot(
        Some(0),
        HashMap::from([("fast".to_string(), 0.1)]),
    ));
    let mut ledger = WorldQiAccount::default();
    ledger
        .set_balance(pending_inflow_account(), 1000.0)
        .expect("seeding pool balance must succeed");
    app.insert_resource(ledger);
    app.add_event::<QiTransfer>();
    app.add_systems(Update, pseudo_vein_fallback_spawn_system);
    for _ in 0..PSEUDO_VEIN_CRITICAL_PLAYER_DENSITY {
        app.world_mut()
            .spawn((Cultivation::default(), Position::new([8.0, 66.0, 8.0])));
    }

    app.update();

    let mut query = app.world_mut().query::<&PseudoVeinRuntime>();
    let runtimes = query.iter(app.world()).collect::<Vec<_>>();
    assert_eq!(runtimes.len(), 1);
    assert_eq!(runtimes[0].zone_id, "fast");
    let expected_absolute = round3((PSEUDO_VEIN_MAX_QI - 0.1) * QI_ZONE_UNIT_CAPACITY);
    assert_eq!(runtimes[0].injected_qi, expected_absolute);
    assert_eq!(
        app.world()
            .resource::<ZoneRegistry>()
            .find_zone_by_name("fast")
            .expect("test zone should exist")
            .spirit_qi,
        PSEUDO_VEIN_MAX_QI
    );
    assert_eq!(
        app.world()
            .resource::<WorldQiAccount>()
            .balance(&pending_inflow_account()),
        round3(1000.0 - expected_absolute),
        "the fallback-spawn path must debit the pending pool by exactly the amount \
         credited to the zone, same conservation invariant as the agent-command path"
    );
}

#[test]
fn fallback_system_spawns_runtime_with_zero_injection_when_pool_is_missing() {
    // ledger resource 缺失（例如某些 headless 测试场景没插入 WorldQiAccount）时，
    // spawn_fallback_pseudo_vein 必须优雅降级为零注入，而不是 panic 或凭空创生。
    let mut app = App::new();
    app.insert_resource(CultivationClock {
        tick: PSEUDO_VEIN_FALLBACK_EVAL_PERIOD_TICKS,
    });
    app.insert_resource(ZoneRegistry {
        spatial_revision: 0,
        zones: vec![zone("fast", 0.1, 0.0)],
    });
    app.insert_resource(PseudoVeinFallbackState::from_test_snapshot(
        Some(0),
        HashMap::from([("fast".to_string(), 0.1)]),
    ));
    app.add_event::<QiTransfer>();
    app.add_systems(Update, pseudo_vein_fallback_spawn_system);
    for _ in 0..PSEUDO_VEIN_CRITICAL_PLAYER_DENSITY {
        app.world_mut()
            .spawn((Cultivation::default(), Position::new([8.0, 66.0, 8.0])));
    }

    app.update();

    let mut query = app.world_mut().query::<&PseudoVeinRuntime>();
    let runtimes = query.iter(app.world()).collect::<Vec<_>>();
    assert_eq!(runtimes.len(), 1, "the runtime should still spawn");
    assert_eq!(
        runtimes[0].injected_qi, 0.0,
        "with no ledger resource available, injection must be a no-op zero, not a crash"
    );
    assert_eq!(
        app.world()
            .resource::<ZoneRegistry>()
            .find_zone_by_name("fast")
            .expect("test zone should exist")
            .spirit_qi,
        0.1,
        "the zone must be left completely untouched when the ledger is unavailable"
    );
}

#[test]
fn visual_cue_matches_runtime_phase() {
    let mut runtime = runtime(PseudoVeinSeasonV1::Summer);
    runtime.advance(600, 0);

    let request = pseudo_vein_vfx_request(&runtime, PseudoVeinPhase::Active);

    match request.payload {
        VfxEventPayloadV1::SpawnParticle {
            event_id,
            color,
            strength,
            ..
        } => {
            assert_eq!(event_id, PSEUDO_VEIN_ACTIVE_VFX_EVENT_ID);
            assert_eq!(color.as_deref(), Some("#FFD36A"));
            assert_eq!(strength, Some(1.0));
        }
        other => panic!("expected pseudo vein SpawnParticle VFX, got {other:?}"),
    }
}

#[test]
fn visual_throttle_emits_on_period_or_phase_change() {
    let mut runtime = runtime(PseudoVeinSeasonV1::Summer);

    assert!(should_emit_visual(
        &mut runtime,
        PseudoVeinPhase::Rising,
        10,
        false
    ));
    assert!(!should_emit_visual(
        &mut runtime,
        PseudoVeinPhase::Rising,
        50,
        false
    ));
    runtime.phase = PseudoVeinPhase::Active;
    assert!(should_emit_visual(
        &mut runtime,
        PseudoVeinPhase::Rising,
        51,
        false
    ));
}

fn runtime(season: PseudoVeinSeasonV1) -> PseudoVeinRuntime {
    PseudoVeinRuntime::new("lingquan_marsh", BlockPos::new(8, 66, 8), 0, season)
}

fn zone(name: &str, spirit_qi: f64, x: f64) -> Zone {
    Zone {
        name: name.to_string(),
        dimension: DimensionKind::Overworld,
        bounds: (DVec3::new(x, 64.0, 0.0), DVec3::new(x + 16.0, 80.0, 16.0)),
        spirit_qi,
        danger_level: 0,
        active_events: Vec::new(),
        patrol_anchors: Vec::new(),
        blocked_tiles: Vec::new(),
        qi_equilibrium: 0.0,
        qi_inflow_per_min: 0.0,
    }
}
