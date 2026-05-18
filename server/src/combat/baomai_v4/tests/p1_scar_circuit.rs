//! P1 疤纹回路测试。

use std::collections::HashSet;

use valence::prelude::{App, Events};

use crate::combat::baomai_v3::events::OverloadMeridianRippleEvent;
use crate::combat::baomai_v4::constants::{
    SCAR_CIRCUIT_CHECK_INTERVAL_TICKS, TRIPLE_YANG_REACH_BONUS,
};
use crate::combat::baomai_v4::events::{
    CircuitBreakReason, ScarCircuitBrokenEvent, ScarCircuitFormedEvent,
};
use crate::combat::baomai_v4::iron_cocoon::IronCocoonTracker;
use crate::combat::baomai_v4::scar_circuit::{ActiveScarCircuits, ScarCircuitKind};
use crate::combat::baomai_v4::scar_history::ScarHistory;
use crate::combat::components::DerivedAttrs;
use crate::combat::events::CombatEvent;
use crate::combat::CombatClock;
use crate::cultivation::components::{MeridianId, MeridianSystem};
use crate::cultivation::meridian::severed::MeridianSeveredPermanent;

fn app_at_tick(tick: u64) -> App {
    let mut app = App::new();
    app.insert_resource(CombatClock { tick });
    app.add_event::<OverloadMeridianRippleEvent>();
    app.add_event::<CombatEvent>();
    crate::combat::baomai_v4::register(&mut app);
    app
}

/// Set meridian integrity for a specific meridian. Also ensures opened = true.
fn set_integrity(ms: &mut MeridianSystem, id: MeridianId, integrity: f64) {
    let m = ms.get_mut(id);
    m.opened = true;
    m.integrity = integrity;
}

/// Spawn an entity with appropriate components for scar circuit testing.
fn spawn_circuit_entity(
    app: &mut App,
    total_overloads: u32,
    integrities: &[(MeridianId, f64)],
) -> valence::prelude::Entity {
    let mut ms = MeridianSystem::default();
    for &(id, integrity) in integrities {
        set_integrity(&mut ms, id, integrity);
    }
    app.world_mut()
        .spawn((
            ScarHistory {
                total_overloads,
                ..Default::default()
            },
            ms,
            ActiveScarCircuits::default(),
            DerivedAttrs::default(),
            IronCocoonTracker::default(),
        ))
        .id()
}

#[test]
fn circuit_forms_when_both_micro_tear() {
    // Both LI and SI at 0.70 (within [0.50, 0.85]) with >= 5 overloads → TigerMouth forms.
    let mut app = app_at_tick(SCAR_CIRCUIT_CHECK_INTERVAL_TICKS); // tick divisible by 40
    let entity = spawn_circuit_entity(
        &mut app,
        10,
        &[
            (MeridianId::LargeIntestine, 0.70),
            (MeridianId::SmallIntestine, 0.70),
        ],
    );

    app.update();

    let circuits = app.world().get::<ActiveScarCircuits>(entity).unwrap();
    assert!(
        circuits.is_active(ScarCircuitKind::TigerMouth),
        "TigerMouth circuit should form when both LI and SI have integrity=0.70"
    );

    // Check event was emitted.
    let events = app.world().resource::<Events<ScarCircuitFormedEvent>>();
    let mut reader = events.get_reader();
    let formed: Vec<_> = reader.read(events).collect();
    assert_eq!(
        formed.len(),
        1,
        "Exactly one ScarCircuitFormedEvent should be emitted"
    );
    assert_eq!(formed[0].circuit, ScarCircuitKind::TigerMouth);
}

#[test]
fn circuit_not_formed_if_one_intact() {
    // LI at 0.70, SI at 0.90 (above INTEGRITY_MAX=0.85) → no circuit.
    let mut app = app_at_tick(SCAR_CIRCUIT_CHECK_INTERVAL_TICKS);
    let entity = spawn_circuit_entity(
        &mut app,
        10,
        &[
            (MeridianId::LargeIntestine, 0.70),
            (MeridianId::SmallIntestine, 0.90),
        ],
    );

    app.update();

    let circuits = app.world().get::<ActiveScarCircuits>(entity).unwrap();
    assert!(
        !circuits.is_active(ScarCircuitKind::TigerMouth),
        "TigerMouth should NOT form when one meridian is above INTEGRITY_MAX (0.90 > 0.85)"
    );
}

#[test]
fn circuit_not_formed_if_one_too_damaged() {
    // LI at 0.70, SI at 0.40 (below INTEGRITY_MIN=0.50) → no circuit.
    let mut app = app_at_tick(SCAR_CIRCUIT_CHECK_INTERVAL_TICKS);
    let entity = spawn_circuit_entity(
        &mut app,
        10,
        &[
            (MeridianId::LargeIntestine, 0.70),
            (MeridianId::SmallIntestine, 0.40),
        ],
    );

    app.update();

    let circuits = app.world().get::<ActiveScarCircuits>(entity).unwrap();
    assert!(
        !circuits.is_active(ScarCircuitKind::TigerMouth),
        "TigerMouth should NOT form when one meridian is below INTEGRITY_MIN (0.40 < 0.50)"
    );
}

#[test]
fn circuit_breaks_on_heal_past_max() {
    // Form circuit, then heal one meridian past INTEGRITY_MAX → circuit breaks.
    let mut app = app_at_tick(SCAR_CIRCUIT_CHECK_INTERVAL_TICKS);
    let entity = spawn_circuit_entity(
        &mut app,
        10,
        &[
            (MeridianId::LargeIntestine, 0.70),
            (MeridianId::SmallIntestine, 0.70),
        ],
    );

    // First update: circuit forms.
    app.update();
    let circuits = app.world().get::<ActiveScarCircuits>(entity).unwrap();
    assert!(circuits.is_active(ScarCircuitKind::TigerMouth));

    // Heal SI to 0.90 (past INTEGRITY_MAX).
    app.world_mut()
        .get_mut::<MeridianSystem>(entity)
        .unwrap()
        .get_mut(MeridianId::SmallIntestine)
        .integrity = 0.90;

    // Advance clock to next check interval.
    app.world_mut().resource_mut::<CombatClock>().tick = SCAR_CIRCUIT_CHECK_INTERVAL_TICKS * 2;
    app.update();

    let circuits = app.world().get::<ActiveScarCircuits>(entity).unwrap();
    assert!(
        !circuits.is_active(ScarCircuitKind::TigerMouth),
        "TigerMouth should break when SI heals past INTEGRITY_MAX"
    );

    // Check break event was emitted with correct reason.
    let events = app.world().resource::<Events<ScarCircuitBrokenEvent>>();
    let mut reader = events.get_reader();
    let broken: Vec<_> = reader.read(events).collect();
    assert!(
        broken
            .iter()
            .any(|e| e.circuit == ScarCircuitKind::TigerMouth
                && e.reason == CircuitBreakReason::Healed),
        "Should emit ScarCircuitBrokenEvent with reason Healed"
    );
}

#[test]
fn circuit_breaks_on_sever() {
    // Form circuit, then SEVER one meridian → circuit breaks.
    let mut app = app_at_tick(SCAR_CIRCUIT_CHECK_INTERVAL_TICKS);
    let entity = spawn_circuit_entity(
        &mut app,
        10,
        &[
            (MeridianId::LargeIntestine, 0.70),
            (MeridianId::SmallIntestine, 0.70),
        ],
    );

    // First update: circuit forms.
    app.update();

    // Insert MeridianSeveredPermanent with LI severed.
    let mut severed = MeridianSeveredPermanent::default();
    severed.severed_meridians.insert(MeridianId::LargeIntestine);
    app.world_mut().entity_mut(entity).insert(severed);

    // Advance clock and update.
    app.world_mut().resource_mut::<CombatClock>().tick = SCAR_CIRCUIT_CHECK_INTERVAL_TICKS * 2;
    app.update();

    let circuits = app.world().get::<ActiveScarCircuits>(entity).unwrap();
    assert!(
        !circuits.is_active(ScarCircuitKind::TigerMouth),
        "TigerMouth should break when LI is SEVERED"
    );
}

#[test]
fn circuit_passive_reach_bonus() {
    // TripleYang active → DerivedAttrs.reach_bonus = 0.3.
    let mut app = app_at_tick(SCAR_CIRCUIT_CHECK_INTERVAL_TICKS);
    let entity = spawn_circuit_entity(
        &mut app,
        10,
        &[
            (MeridianId::SmallIntestine, 0.70),
            (MeridianId::TripleEnergizer, 0.70),
        ],
    );

    app.update();

    let attrs = app.world().get::<DerivedAttrs>(entity).unwrap();
    assert!(
        (attrs.reach_bonus - TRIPLE_YANG_REACH_BONUS).abs() < 1e-6,
        "TripleYang active should set reach_bonus to {}, got {}",
        TRIPLE_YANG_REACH_BONUS,
        attrs.reach_bonus
    );
}

#[test]
fn circuit_passive_no_stack() {
    // Only one reach bonus source (TripleYang). No stacking needed — just verify
    // the value doesn't double.
    let mut app = app_at_tick(SCAR_CIRCUIT_CHECK_INTERVAL_TICKS);
    let entity = spawn_circuit_entity(
        &mut app,
        10,
        &[
            (MeridianId::SmallIntestine, 0.70),
            (MeridianId::TripleEnergizer, 0.70),
        ],
    );

    // Run twice to verify no accumulation.
    app.update();
    app.world_mut().resource_mut::<CombatClock>().tick = SCAR_CIRCUIT_CHECK_INTERVAL_TICKS * 2;
    app.update();

    let attrs = app.world().get::<DerivedAttrs>(entity).unwrap();
    assert!(
        (attrs.reach_bonus - TRIPLE_YANG_REACH_BONUS).abs() < 1e-6,
        "Reach bonus should not stack across multiple ticks. Expected {}, got {}",
        TRIPLE_YANG_REACH_BONUS,
        attrs.reach_bonus
    );
}

#[test]
fn circuit_requires_min_overloads() {
    // total_overloads = 3 (< SCAR_CIRCUIT_MIN_OVERLOADS=5) → no circuit even if integrity is right.
    let mut app = app_at_tick(SCAR_CIRCUIT_CHECK_INTERVAL_TICKS);
    let entity = spawn_circuit_entity(
        &mut app,
        3,
        &[
            (MeridianId::LargeIntestine, 0.70),
            (MeridianId::SmallIntestine, 0.70),
        ],
    );

    app.update();

    let circuits = app.world().get::<ActiveScarCircuits>(entity).unwrap();
    assert!(
        !circuits.is_active(ScarCircuitKind::TigerMouth),
        "TigerMouth should NOT form when total_overloads={} < MIN_OVERLOADS=5",
        3
    );
}

#[test]
fn circuit_multiple_active() {
    // Can have multiple circuits active simultaneously (different limbs).
    let mut app = app_at_tick(SCAR_CIRCUIT_CHECK_INTERVAL_TICKS);
    let entity = spawn_circuit_entity(
        &mut app,
        10,
        &[
            // Right arm: TigerMouth (LI↔SI)
            (MeridianId::LargeIntestine, 0.70),
            (MeridianId::SmallIntestine, 0.70),
            // Left arm: HeartLung (LU↔HT)
            (MeridianId::Lung, 0.70),
            (MeridianId::Heart, 0.70),
            // Right leg: GallStomach (ST↔GB)
            (MeridianId::Stomach, 0.60),
            (MeridianId::Gallbladder, 0.60),
        ],
    );

    app.update();

    let circuits = app.world().get::<ActiveScarCircuits>(entity).unwrap();
    assert!(circuits.is_active(ScarCircuitKind::TigerMouth));
    assert!(circuits.is_active(ScarCircuitKind::HeartLung));
    assert!(circuits.is_active(ScarCircuitKind::GallStomach));
    assert_eq!(
        circuits.active_count(),
        3,
        "Should have 3 active circuits simultaneously"
    );
}

#[test]
fn circuit_check_period() {
    // At tick not divisible by 40, no check occurs.
    let mut app = app_at_tick(SCAR_CIRCUIT_CHECK_INTERVAL_TICKS + 1); // Not divisible by 40
    let entity = spawn_circuit_entity(
        &mut app,
        10,
        &[
            (MeridianId::LargeIntestine, 0.70),
            (MeridianId::SmallIntestine, 0.70),
        ],
    );

    app.update();

    let circuits = app.world().get::<ActiveScarCircuits>(entity).unwrap();
    assert!(
        !circuits.is_active(ScarCircuitKind::TigerMouth),
        "Circuit check should only run at ticks divisible by {}, tick {} should skip",
        SCAR_CIRCUIT_CHECK_INTERVAL_TICKS,
        SCAR_CIRCUIT_CHECK_INTERVAL_TICKS + 1
    );
}

#[test]
fn circuit_boundary_integrity_min() {
    // Both meridians at exactly INTEGRITY_MIN (0.50) → should form (inclusive).
    let mut app = app_at_tick(SCAR_CIRCUIT_CHECK_INTERVAL_TICKS);
    let entity = spawn_circuit_entity(
        &mut app,
        10,
        &[
            (MeridianId::LargeIntestine, 0.50),
            (MeridianId::SmallIntestine, 0.50),
        ],
    );

    app.update();

    let circuits = app.world().get::<ActiveScarCircuits>(entity).unwrap();
    assert!(
        circuits.is_active(ScarCircuitKind::TigerMouth),
        "Circuit should form at exactly INTEGRITY_MIN=0.50 (inclusive boundary)"
    );
}

#[test]
fn circuit_boundary_integrity_max() {
    // Both meridians at exactly INTEGRITY_MAX (0.85) → should form (inclusive).
    let mut app = app_at_tick(SCAR_CIRCUIT_CHECK_INTERVAL_TICKS);
    let entity = spawn_circuit_entity(
        &mut app,
        10,
        &[
            (MeridianId::LargeIntestine, 0.85),
            (MeridianId::SmallIntestine, 0.85),
        ],
    );

    app.update();

    let circuits = app.world().get::<ActiveScarCircuits>(entity).unwrap();
    assert!(
        circuits.is_active(ScarCircuitKind::TigerMouth),
        "Circuit should form at exactly INTEGRITY_MAX=0.85 (inclusive boundary)"
    );
}

#[test]
fn circuit_not_formed_if_meridian_not_opened() {
    // Meridian not opened → no circuit even if integrity is in range.
    let mut app = app_at_tick(SCAR_CIRCUIT_CHECK_INTERVAL_TICKS);
    let mut ms = MeridianSystem::default();
    // Only open LI, leave SI closed but set its integrity in range.
    set_integrity(&mut ms, MeridianId::LargeIntestine, 0.70);
    ms.get_mut(MeridianId::SmallIntestine).integrity = 0.70;
    // SI.opened remains false.

    let entity = app
        .world_mut()
        .spawn((
            ScarHistory {
                total_overloads: 10,
                ..Default::default()
            },
            ms,
            ActiveScarCircuits::default(),
            DerivedAttrs::default(),
            IronCocoonTracker::default(),
        ))
        .id();

    app.update();

    let circuits = app.world().get::<ActiveScarCircuits>(entity).unwrap();
    assert!(
        !circuits.is_active(ScarCircuitKind::TigerMouth),
        "Circuit should NOT form when one meridian is not opened"
    );
}

#[test]
fn circuit_derive_resets_on_break() {
    // When circuit breaks, DerivedAttrs fields should reset to neutral.
    let mut app = app_at_tick(SCAR_CIRCUIT_CHECK_INTERVAL_TICKS);
    let entity = spawn_circuit_entity(
        &mut app,
        10,
        &[
            (MeridianId::SmallIntestine, 0.70),
            (MeridianId::TripleEnergizer, 0.70),
        ],
    );

    // Form circuit.
    app.update();
    let attrs = app.world().get::<DerivedAttrs>(entity).unwrap();
    assert!(
        attrs.reach_bonus > 0.0,
        "Should have reach bonus when TripleYang active"
    );

    // Break circuit by healing.
    app.world_mut()
        .get_mut::<MeridianSystem>(entity)
        .unwrap()
        .get_mut(MeridianId::SmallIntestine)
        .integrity = 0.95;

    app.world_mut().resource_mut::<CombatClock>().tick = SCAR_CIRCUIT_CHECK_INTERVAL_TICKS * 2;
    app.update();

    let attrs = app.world().get::<DerivedAttrs>(entity).unwrap();
    assert!(
        attrs.reach_bonus.abs() < 1e-6,
        "reach_bonus should reset to 0.0 when TripleYang circuit breaks, got {}",
        attrs.reach_bonus
    );
}

#[test]
fn all_seven_circuit_kinds_have_valid_pairs() {
    // Verify every ScarCircuitKind maps to a valid adjacency pair.
    for kind in ScarCircuitKind::ALL {
        let (a, b) = kind.meridian_pair();
        assert!(
            crate::combat::baomai_v4::adjacency::are_adjacent(a, b),
            "Circuit {:?} pair ({:?}, {:?}) should be adjacent in MERIDIAN_ADJACENCY",
            kind,
            a,
            b
        );
    }
}

#[test]
fn circuit_kinds_cover_all_adjacency_pairs_subset() {
    // ScarCircuitKind covers 7 out of 13 adjacency pairs (design decision: not all pairs
    // have meaningful circuit effects). Just verify each kind maps to a unique pair.
    let mut pairs: HashSet<(MeridianId, MeridianId)> = HashSet::new();
    for kind in ScarCircuitKind::ALL {
        let pair = kind.meridian_pair();
        assert!(
            pairs.insert(pair),
            "Duplicate meridian pair for circuit kind {:?}",
            kind
        );
    }
    assert_eq!(pairs.len(), 7, "Should have exactly 7 unique circuit pairs");
}
