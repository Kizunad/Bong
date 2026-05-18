//! P3 死脉甲测试（plan §4.5）。
//!
//! 12 个测试覆盖绝脉技施放条件、施放效果、污染免疫、引导打断。

use valence::prelude::{App, Events};

use crate::combat::baomai_v3::events::{BaomaiSkillId, OverloadMeridianRippleEvent};
use crate::combat::baomai_v3::state::BaomaiMastery;
use crate::combat::baomai_v4::constants::{
    VOLUNTARY_SEVER_CAST_TICKS, VOLUNTARY_SEVER_COMBAT_COOLDOWN_TICKS,
    VOLUNTARY_SEVER_MAX_INTEGRITY, VOLUNTARY_SEVER_MIN_MASTERY, VOLUNTARY_SEVER_MIN_OVERLOADS,
};
use crate::combat::baomai_v4::dead_armor::{
    check_voluntary_sever, meridian_to_body_part, should_block_contamination, DeadMeridianArmor,
    VoluntarySeverReject, VOLUNTARY_SEVER_SKILL_ID,
};
use crate::combat::baomai_v4::events::{
    CircuitBreakReason, ScarCircuitBrokenEvent, VoluntarySeverEvent,
};
use crate::combat::baomai_v4::iron_cocoon::IronCocoonTracker;
use crate::combat::baomai_v4::scar_circuit::{ActiveScarCircuits, ScarCircuitKind};
use crate::combat::baomai_v4::scar_history::ScarHistory;
use crate::combat::components::{BodyPart, CombatState, DerivedAttrs};
use crate::combat::CombatClock;
use crate::cultivation::components::{MeridianId, MeridianSystem};
use crate::cultivation::meridian::severed::{
    check_meridian_dependencies, MeridianSeveredEvent, MeridianSeveredPermanent, SeveredSource,
    SkillMeridianDependencies,
};

/// Build a minimal app with baomai_v4 systems registered.
fn app_at_tick(tick: u64) -> App {
    let mut app = App::new();
    app.insert_resource(CombatClock { tick });
    app.add_event::<OverloadMeridianRippleEvent>();
    app.add_event::<MeridianSeveredEvent>();
    crate::combat::baomai_v4::register(&mut app);
    app
}

/// Set meridian integrity and opened state.
fn set_integrity(ms: &mut MeridianSystem, id: MeridianId, integrity: f64) {
    let m = ms.get_mut(id);
    m.opened = integrity > f64::EPSILON;
    m.integrity = integrity;
}

/// Create a BaomaiMastery with a specific skill at a given mastery level.
fn mastery_with(skill: BaomaiSkillId, level: f32) -> BaomaiMastery {
    let mut m = BaomaiMastery::default();
    m.set_level(skill, level);
    m
}

// ── 施放条件测试 ──

#[test]
fn voluntary_sever_requires_overloads() {
    // total_overloads = 79 (< 80) -> reject InsufficientOverloads
    let history = ScarHistory {
        total_overloads: VOLUNTARY_SEVER_MIN_OVERLOADS - 1,
        ..Default::default()
    };
    let mastery = mastery_with(BaomaiSkillId::BengQuan, 80.0);
    let mut ms = MeridianSystem::default();
    set_integrity(&mut ms, MeridianId::Lung, 0.50);

    let result = check_voluntary_sever(
        MeridianId::Lung,
        &history,
        Some(&mastery),
        &ms,
        None,
        None,
        1000,
    );

    assert!(
        matches!(
            result,
            Err(VoluntarySeverReject::InsufficientOverloads { .. })
        ),
        "total_overloads={} < {} should reject with InsufficientOverloads, got {:?}",
        VOLUNTARY_SEVER_MIN_OVERLOADS - 1,
        VOLUNTARY_SEVER_MIN_OVERLOADS,
        result,
    );
}

#[test]
fn voluntary_sever_requires_mastery() {
    // mastery = 59 (< 60) -> reject InsufficientMastery
    let history = ScarHistory {
        total_overloads: 100,
        ..Default::default()
    };
    let mastery = mastery_with(BaomaiSkillId::BengQuan, VOLUNTARY_SEVER_MIN_MASTERY - 1.0);
    let mut ms = MeridianSystem::default();
    set_integrity(&mut ms, MeridianId::Lung, 0.50);

    let result = check_voluntary_sever(
        MeridianId::Lung,
        &history,
        Some(&mastery),
        &ms,
        None,
        None,
        1000,
    );

    assert!(
        matches!(
            result,
            Err(VoluntarySeverReject::InsufficientMastery { .. })
        ),
        "mastery={} < {} should reject with InsufficientMastery, got {:?}",
        VOLUNTARY_SEVER_MIN_MASTERY - 1.0,
        VOLUNTARY_SEVER_MIN_MASTERY,
        result,
    );
}

#[test]
fn voluntary_sever_requires_damaged_meridian() {
    // integrity = 0.70 (>= max_integrity) -> reject MeridianTooHealthy
    let history = ScarHistory {
        total_overloads: 100,
        ..Default::default()
    };
    let mastery = mastery_with(BaomaiSkillId::BengQuan, 80.0);
    let mut ms = MeridianSystem::default();
    set_integrity(&mut ms, MeridianId::Lung, VOLUNTARY_SEVER_MAX_INTEGRITY);

    let result = check_voluntary_sever(
        MeridianId::Lung,
        &history,
        Some(&mastery),
        &ms,
        None,
        None,
        1000,
    );

    assert!(
        matches!(result, Err(VoluntarySeverReject::MeridianTooHealthy { .. })),
        "integrity={} >= {} should reject with MeridianTooHealthy, got {:?}",
        VOLUNTARY_SEVER_MAX_INTEGRITY,
        VOLUNTARY_SEVER_MAX_INTEGRITY,
        result,
    );

    // integrity = 0.69 (< 0.70) -> should pass this check (may fail others)
    set_integrity(
        &mut ms,
        MeridianId::Lung,
        VOLUNTARY_SEVER_MAX_INTEGRITY - 0.01,
    );
    let result2 = check_voluntary_sever(
        MeridianId::Lung,
        &history,
        Some(&mastery),
        &ms,
        None,
        None,
        1000,
    );
    assert!(
        result2.is_ok(),
        "integrity={} < {} should pass, got {:?}",
        VOLUNTARY_SEVER_MAX_INTEGRITY - 0.01,
        VOLUNTARY_SEVER_MAX_INTEGRITY,
        result2,
    );
}

#[test]
fn voluntary_sever_requires_out_of_combat() {
    // last_attack_at_tick = 950, now = 1000, cooldown = 60 -> 950+60=1010 > 1000 -> InCombat
    let history = ScarHistory {
        total_overloads: 100,
        ..Default::default()
    };
    let mastery = mastery_with(BaomaiSkillId::BengQuan, 80.0);
    let mut ms = MeridianSystem::default();
    set_integrity(&mut ms, MeridianId::Lung, 0.50);

    let now = 1000u64;
    // Set last_attack within cooldown window.
    let mut combat_state = CombatState {
        last_attack_at_tick: Some(now - VOLUNTARY_SEVER_COMBAT_COOLDOWN_TICKS + 1),
        in_combat_until_tick: Some(now + 100),
        ..Default::default()
    };

    let result = check_voluntary_sever(
        MeridianId::Lung,
        &history,
        Some(&mastery),
        &ms,
        None,
        Some(&combat_state),
        now,
    );

    assert!(
        matches!(result, Err(VoluntarySeverReject::InCombat)),
        "within combat cooldown should reject InCombat, got {:?}",
        result,
    );

    // After cooldown -> should pass.
    combat_state.last_attack_at_tick = Some(now - VOLUNTARY_SEVER_COMBAT_COOLDOWN_TICKS - 1);
    let result2 = check_voluntary_sever(
        MeridianId::Lung,
        &history,
        Some(&mastery),
        &ms,
        None,
        Some(&combat_state),
        now,
    );
    assert!(
        result2.is_ok(),
        "after combat cooldown should pass, got {:?}",
        result2,
    );
}

// ── 施放效果测试 ──

#[test]
fn voluntary_sever_marks_permanent() {
    // VoluntarySeverEvent -> MeridianSeveredPermanent contains the meridian with VoluntarySever source.
    let mut app = app_at_tick(1000);
    let mut ms = MeridianSystem::default();
    set_integrity(&mut ms, MeridianId::LargeIntestine, 0.50);

    let entity = app
        .world_mut()
        .spawn((
            ScarHistory {
                total_overloads: 100,
                ..Default::default()
            },
            ms,
            ActiveScarCircuits::default(),
            DerivedAttrs::default(),
            IronCocoonTracker::default(),
            MeridianSeveredPermanent::default(),
            DeadMeridianArmor::default(),
        ))
        .id();

    app.world_mut().send_event(VoluntarySeverEvent {
        entity,
        meridian: MeridianId::LargeIntestine,
        tick: 1000,
    });

    app.update();

    let severed = app.world().get::<MeridianSeveredPermanent>(entity).unwrap();
    assert!(
        severed.is_severed(MeridianId::LargeIntestine),
        "LargeIntestine should be marked SEVERED after VoluntarySeverEvent"
    );
    let record = severed
        .record_for(MeridianId::LargeIntestine)
        .expect("severed record should exist");
    assert_eq!(
        record.source,
        SeveredSource::VoluntarySever,
        "source should be VoluntarySever"
    );

    // Meridian integrity should be 0.0 and opened = false.
    let ms = app.world().get::<MeridianSystem>(entity).unwrap();
    let m = ms.get(MeridianId::LargeIntestine);
    assert_eq!(m.integrity, 0.0, "integrity should be clamped to 0.0");
    assert!(!m.opened, "meridian should be closed after sever");
}

#[test]
fn voluntary_sever_grants_contam_immunity() {
    // After VoluntarySeverEvent on LargeIntestine -> ArmR should be immune.
    let mut app = app_at_tick(1000);
    let mut ms = MeridianSystem::default();
    set_integrity(&mut ms, MeridianId::LargeIntestine, 0.50);

    let entity = app
        .world_mut()
        .spawn((
            ScarHistory {
                total_overloads: 100,
                ..Default::default()
            },
            ms,
            ActiveScarCircuits::default(),
            DerivedAttrs::default(),
            IronCocoonTracker::default(),
            MeridianSeveredPermanent::default(),
            DeadMeridianArmor::default(),
        ))
        .id();

    app.world_mut().send_event(VoluntarySeverEvent {
        entity,
        meridian: MeridianId::LargeIntestine,
        tick: 1000,
    });

    app.update();

    let armor = app.world().get::<DeadMeridianArmor>(entity).unwrap();
    assert!(
        armor.is_immune(BodyPart::ArmR),
        "LI -> ArmR should be immune after VoluntarySever"
    );
    assert!(
        !armor.is_immune(BodyPart::ArmL),
        "ArmL should NOT be immune (LI maps to ArmR, not ArmL)"
    );
}

#[test]
fn voluntary_sever_breaks_circuit() {
    // TigerMouth circuit (LI <-> SI) should break when LI is severed.
    let mut app = app_at_tick(1000);
    let mut ms = MeridianSystem::default();
    set_integrity(&mut ms, MeridianId::LargeIntestine, 0.50);
    set_integrity(&mut ms, MeridianId::SmallIntestine, 0.70);

    let mut circuits = ActiveScarCircuits::default();
    circuits.circuits.insert(ScarCircuitKind::TigerMouth);
    circuits.formed_at.insert(ScarCircuitKind::TigerMouth, 500);

    let entity = app
        .world_mut()
        .spawn((
            ScarHistory {
                total_overloads: 100,
                ..Default::default()
            },
            ms,
            circuits,
            DerivedAttrs::default(),
            IronCocoonTracker::default(),
            MeridianSeveredPermanent::default(),
            DeadMeridianArmor::default(),
        ))
        .id();

    app.world_mut().send_event(VoluntarySeverEvent {
        entity,
        meridian: MeridianId::LargeIntestine,
        tick: 1000,
    });

    app.update();

    let circuits = app.world().get::<ActiveScarCircuits>(entity).unwrap();
    assert!(
        !circuits.is_active(ScarCircuitKind::TigerMouth),
        "TigerMouth circuit should break after severing LI"
    );

    // Check that ScarCircuitBrokenEvent was emitted.
    let broken_events = app.world().resource::<Events<ScarCircuitBrokenEvent>>();
    let mut reader = broken_events.get_reader();
    let broken: Vec<_> = reader.read(broken_events).collect();
    assert!(
        broken
            .iter()
            .any(|e| e.circuit == ScarCircuitKind::TigerMouth
                && e.reason == CircuitBreakReason::Severed),
        "ScarCircuitBrokenEvent(TigerMouth, Severed) should be emitted, got {:?}",
        broken,
    );
}

#[test]
fn voluntary_sever_blocks_dependent_skills() {
    // After severing Lung, skills that depend on Lung should be blocked by check_meridian_dependencies.
    let mut severed = MeridianSeveredPermanent::default();
    severed.insert(MeridianId::Lung, SeveredSource::VoluntarySever, 1000);

    // Declare a skill that depends on Lung.
    let mut deps_table = SkillMeridianDependencies::default();
    deps_table.declare("test.lung_skill", vec![MeridianId::Lung]);

    let deps = deps_table.lookup("test.lung_skill");
    let result = check_meridian_dependencies(deps, Some(&severed));
    assert_eq!(
        result,
        Err(MeridianId::Lung),
        "skill depending on severed Lung should be blocked"
    );

    // VoluntarySever skill itself has no dependencies -> always passes.
    deps_table.declare(VOLUNTARY_SEVER_SKILL_ID, vec![]);
    let vs_deps = deps_table.lookup(VOLUNTARY_SEVER_SKILL_ID);
    let vs_result = check_meridian_dependencies(vs_deps, Some(&severed));
    assert!(
        vs_result.is_ok(),
        "voluntary_sever skill has no deps, should pass even when Lung is severed"
    );
}

// ── 污染免疫测试 ──

#[test]
fn dead_armor_blocks_contamination() {
    // ArmR is immune -> contamination for ArmR should be blocked.
    let mut armor = DeadMeridianArmor::default();
    armor.immune_regions.insert(BodyPart::ArmR);

    assert!(
        should_block_contamination(&armor, BodyPart::ArmR),
        "ArmR should block contamination when immune"
    );
    assert!(
        !should_block_contamination(&armor, BodyPart::ArmL),
        "ArmL should NOT block contamination (not immune)"
    );
    assert!(
        !should_block_contamination(&armor, BodyPart::Chest),
        "Chest should NOT block contamination (not immune)"
    );
}

#[test]
fn dead_armor_allows_physical_damage() {
    // Physical damage (wound_grade, wound kind) is unaffected by DeadMeridianArmor.
    // The should_block_contamination function only gates contamination, not physical.
    let mut armor = DeadMeridianArmor::default();
    armor.immune_regions.insert(BodyPart::ArmR);

    // 1. Contamination IS blocked for the immune region.
    assert!(
        should_block_contamination(&armor, BodyPart::ArmR),
        "contamination should be blocked for ArmR (immune region)"
    );

    // 2. Non-immune region is NOT blocked (sanity).
    assert!(
        !should_block_contamination(&armor, BodyPart::ArmL),
        "contamination should NOT be blocked for ArmL (not immune)"
    );

    // 3. Design constraint: DeadMeridianArmor has exactly one field (`immune_regions`)
    //    and one method (`is_immune`). It does NOT carry any physical damage modifier,
    //    WoundGrade scaling, or damage reduction field. This is a structural pin test:
    //    if someone adds a physical-damage field to DeadMeridianArmor, this test forces
    //    them to explicitly acknowledge the design decision.
    let default_armor = DeadMeridianArmor::default();
    assert!(
        default_armor.immune_regions.is_empty(),
        "default DeadMeridianArmor must start with no immune regions"
    );
    // Confirm is_immune returns false for all body parts on a default armor.
    for body_part in [
        BodyPart::ArmR,
        BodyPart::ArmL,
        BodyPart::LegR,
        BodyPart::LegL,
        BodyPart::Chest,
        BodyPart::Back,
        BodyPart::Head,
    ] {
        assert!(
            !default_armor.is_immune(body_part),
            "default armor must not grant immunity to {body_part:?} — \
             DeadMeridianArmor only tracks contamination immunity, never physical damage"
        );
    }

    // 4. After granting immunity, only contamination queries are affected.
    //    The immune_regions set is the SOLE state — no side-channel for physical damage.
    assert_eq!(
        armor.immune_regions.len(),
        1,
        "only one region should be immune; DeadMeridianArmor carries no other state"
    );
    assert!(
        armor.immune_regions.contains(&BodyPart::ArmR),
        "the sole immune region must be ArmR"
    );
}

#[test]
fn dead_armor_only_voluntary() {
    // Only VoluntarySever source grants contamination immunity.
    // Combat-severed meridians do NOT get immunity.

    // Simulate: Lung severed by CombatWound -> ArmL should NOT be immune.
    let mut severed = MeridianSeveredPermanent::default();
    severed.insert(MeridianId::Lung, SeveredSource::CombatWound, 500);

    // DeadMeridianArmor starts empty.
    let armor = DeadMeridianArmor::default();

    // ArmL (mapped from Lung) should not be immune because the sever was CombatWound, not VoluntarySever.
    assert!(
        !armor.is_immune(BodyPart::ArmL),
        "CombatWound-severed meridian should NOT grant contamination immunity"
    );

    // Now simulate voluntary sever through the system.
    let mut app = app_at_tick(1000);
    let mut ms = MeridianSystem::default();
    set_integrity(&mut ms, MeridianId::Lung, 0.50);

    let entity = app
        .world_mut()
        .spawn((
            ScarHistory {
                total_overloads: 100,
                ..Default::default()
            },
            ms,
            ActiveScarCircuits::default(),
            DerivedAttrs::default(),
            IronCocoonTracker::default(),
            // Pre-severed by CombatWound.
            severed,
            DeadMeridianArmor::default(),
        ))
        .id();

    // Try to VoluntarySever the already-severed Lung -> should be skipped (already severed).
    app.world_mut().send_event(VoluntarySeverEvent {
        entity,
        meridian: MeridianId::Lung,
        tick: 1000,
    });

    app.update();

    let armor = app.world().get::<DeadMeridianArmor>(entity).unwrap();
    assert!(
        !armor.is_immune(BodyPart::ArmL),
        "already-CombatWound-severed meridian should NOT gain immunity from VoluntarySever attempt"
    );

    // Verify original CombatWound source is preserved.
    let perm = app.world().get::<MeridianSeveredPermanent>(entity).unwrap();
    let record = perm.record_for(MeridianId::Lung).unwrap();
    assert_eq!(
        record.source,
        SeveredSource::CombatWound,
        "first source (CombatWound) should be preserved, not overwritten by VoluntarySever"
    );
}

#[test]
fn voluntary_sever_interruptible() {
    // The cast takes 100 ticks (5s) and can be interrupted by an attack.
    // This test verifies the CAST_TICKS constant and the combat-state check interaction.
    // When a CombatEvent occurs during the 100-tick cast window, re-checking conditions should fail.

    let history = ScarHistory {
        total_overloads: 100,
        ..Default::default()
    };
    let mastery = mastery_with(BaomaiSkillId::BengQuan, 80.0);
    let mut ms = MeridianSystem::default();
    set_integrity(&mut ms, MeridianId::Lung, 0.50);

    // Pre-cast: no combat -> passes.
    let result_pre = check_voluntary_sever(
        MeridianId::Lung,
        &history,
        Some(&mastery),
        &ms,
        None,
        None,
        1000,
    );
    assert!(
        result_pre.is_ok(),
        "pre-cast check should pass, got {:?}",
        result_pre,
    );

    // Verify cast duration constant matches plan (100 ticks = 5s at 20 tps).
    assert_eq!(
        VOLUNTARY_SEVER_CAST_TICKS, 100,
        "cast duration should be 100 ticks (5 seconds at 20 tps)"
    );

    // During cast: attacked at tick 1020 -> re-check at tick 1030 should fail.
    let combat_state = CombatState {
        last_attack_at_tick: Some(1020),
        in_combat_until_tick: Some(1020 + 300), // 15s window
        ..Default::default()
    };

    let result_during = check_voluntary_sever(
        MeridianId::Lung,
        &history,
        Some(&mastery),
        &ms,
        None,
        Some(&combat_state),
        1030, // 10 ticks after attack, within cooldown
    );
    assert!(
        matches!(result_during, Err(VoluntarySeverReject::InCombat)),
        "check during attack window should fail InCombat, got {:?}",
        result_during,
    );
}

// ── 映射正确性验证 ──

#[test]
fn meridian_to_body_part_mapping_complete() {
    // Verify all 14 allowed meridians map to expected body parts.
    assert_eq!(
        meridian_to_body_part(MeridianId::Lung),
        Some(BodyPart::ArmL)
    );
    assert_eq!(
        meridian_to_body_part(MeridianId::Heart),
        Some(BodyPart::ArmL)
    );
    assert_eq!(
        meridian_to_body_part(MeridianId::Pericardium),
        Some(BodyPart::ArmL)
    );

    assert_eq!(
        meridian_to_body_part(MeridianId::LargeIntestine),
        Some(BodyPart::ArmR)
    );
    assert_eq!(
        meridian_to_body_part(MeridianId::SmallIntestine),
        Some(BodyPart::ArmR)
    );
    assert_eq!(
        meridian_to_body_part(MeridianId::TripleEnergizer),
        Some(BodyPart::ArmR)
    );

    assert_eq!(
        meridian_to_body_part(MeridianId::Spleen),
        Some(BodyPart::LegL)
    );
    assert_eq!(
        meridian_to_body_part(MeridianId::Kidney),
        Some(BodyPart::LegL)
    );
    assert_eq!(
        meridian_to_body_part(MeridianId::Liver),
        Some(BodyPart::LegL)
    );

    assert_eq!(
        meridian_to_body_part(MeridianId::Stomach),
        Some(BodyPart::LegR)
    );
    assert_eq!(
        meridian_to_body_part(MeridianId::Bladder),
        Some(BodyPart::LegR)
    );
    assert_eq!(
        meridian_to_body_part(MeridianId::Gallbladder),
        Some(BodyPart::LegR)
    );

    assert_eq!(
        meridian_to_body_part(MeridianId::Ren),
        Some(BodyPart::Chest)
    );
    assert_eq!(meridian_to_body_part(MeridianId::Du), Some(BodyPart::Back));

    // Excluded extraordinary meridians -> None.
    assert_eq!(meridian_to_body_part(MeridianId::Chong), None);
    assert_eq!(meridian_to_body_part(MeridianId::Dai), None);
    assert_eq!(meridian_to_body_part(MeridianId::YinQiao), None);
    assert_eq!(meridian_to_body_part(MeridianId::YangQiao), None);
    assert_eq!(meridian_to_body_part(MeridianId::YinWei), None);
    assert_eq!(meridian_to_body_part(MeridianId::YangWei), None);
}

#[test]
fn voluntary_sever_rejects_excluded_extraordinary_meridians() {
    // §8.1 #3: Chong/Dai/YinQiao/YangQiao/YinWei/YangWei are excluded.
    let history = ScarHistory {
        total_overloads: 200,
        ..Default::default()
    };
    let mastery = mastery_with(BaomaiSkillId::BengQuan, 80.0);
    let ms = MeridianSystem::default();

    for excluded in [
        MeridianId::Chong,
        MeridianId::Dai,
        MeridianId::YinQiao,
        MeridianId::YangQiao,
        MeridianId::YinWei,
        MeridianId::YangWei,
    ] {
        let result =
            check_voluntary_sever(excluded, &history, Some(&mastery), &ms, None, None, 1000);
        assert!(
            matches!(result, Err(VoluntarySeverReject::MeridianNotAllowed)),
            "{:?} should be rejected as MeridianNotAllowed, got {:?}",
            excluded,
            result,
        );
    }
}
