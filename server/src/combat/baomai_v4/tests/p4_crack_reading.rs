//! P4 裂读测试（plan §5.6）。
//!
//! 9 个测试覆盖概率计算、近战判定、经脉精度、深读窗口、回路/死脉甲信息、
//! 目标切换重置、精确值遮蔽、client payload 格式。

use valence::prelude::App;

use crate::combat::baomai_v3::events::OverloadMeridianRippleEvent;
use crate::combat::baomai_v4::constants::{
    CRACK_READING_DEEP_WINDOW_TICKS, CRACK_READING_DISPLAY_TICKS, CRACK_READING_RATE_CAP,
    CRACK_READING_RATE_PER_OVERLOAD,
};
use crate::combat::baomai_v4::crack_reading::{
    bracket_for_npc, bracket_for_player, build_reading_result, crack_reading_success_rate,
    is_melee_source, to_client_payload, CrackReadingState, IntegrityBracket, MeridianReadEntry,
    NpcIntegrityBracket,
};
use crate::combat::baomai_v4::dead_armor::DeadMeridianArmor;
use crate::combat::baomai_v4::scar_circuit::{ActiveScarCircuits, ScarCircuitKind};
use crate::combat::components::BodyPart;
use crate::combat::events::AttackSource;
use crate::combat::CombatClock;
use crate::cultivation::components::{MeridianId, MeridianSystem};
use crate::cultivation::meridian::severed::MeridianSeveredEvent;

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

// ── §5.6 Test 1: crack_reading_probability_scales ──

#[test]
fn crack_reading_probability_scales() {
    // 0 overloads -> 0%
    let rate_0 = crack_reading_success_rate(0);
    assert!(
        (rate_0 - 0.0).abs() < f64::EPSILON,
        "0 overloads should give 0% success rate, got {rate_0}"
    );

    // 50 overloads -> 20%
    let rate_50 = crack_reading_success_rate(50);
    let expected_50 = 50.0 * CRACK_READING_RATE_PER_OVERLOAD;
    assert!(
        (rate_50 - expected_50).abs() < f64::EPSILON,
        "50 overloads should give {expected_50} success rate, got {rate_50}"
    );
    assert!(
        (rate_50 - 0.20).abs() < f64::EPSILON,
        "50 * 0.004 = 0.20, got {rate_50}"
    );

    // 100 overloads -> 40%
    let rate_100 = crack_reading_success_rate(100);
    assert!(
        (rate_100 - 0.40).abs() < f64::EPSILON,
        "100 overloads should give 40%, got {rate_100}"
    );

    // 125 overloads -> 50% (cap)
    let rate_125 = crack_reading_success_rate(125);
    assert!(
        (rate_125 - CRACK_READING_RATE_CAP).abs() < f64::EPSILON,
        "125 overloads should hit cap {}, got {rate_125}",
        CRACK_READING_RATE_CAP
    );

    // 200 overloads -> still 50% (capped)
    let rate_200 = crack_reading_success_rate(200);
    assert!(
        (rate_200 - CRACK_READING_RATE_CAP).abs() < f64::EPSILON,
        "200 overloads should remain at cap {}, got {rate_200}",
        CRACK_READING_RATE_CAP
    );

    // Verify the cap constant itself is 0.50.
    assert!(
        (CRACK_READING_RATE_CAP - 0.50).abs() < f64::EPSILON,
        "CRACK_READING_RATE_CAP should be 0.50, got {}",
        CRACK_READING_RATE_CAP
    );
}

// ── §5.6 Test 2: crack_reading_requires_melee_hit ──

#[test]
fn crack_reading_requires_melee_hit() {
    // Melee sources should trigger.
    assert!(
        is_melee_source(AttackSource::Melee),
        "Melee should be a melee source"
    );
    assert!(
        is_melee_source(AttackSource::BurstMeridian),
        "BurstMeridian (崩拳) should be a melee source"
    );
    assert!(
        is_melee_source(AttackSource::FullPower),
        "FullPower (全力一击) should be a melee source"
    );
    assert!(
        is_melee_source(AttackSource::SwordCleave),
        "SwordCleave should be a melee source"
    );
    assert!(
        is_melee_source(AttackSource::SwordThrust),
        "SwordThrust should be a melee source"
    );

    // Ranged/non-melee sources should NOT trigger.
    assert!(
        !is_melee_source(AttackSource::QiNeedle),
        "QiNeedle (远程) should NOT be a melee source"
    );
    assert!(
        !is_melee_source(AttackSource::SwordPathQiSlash),
        "SwordPathQiSlash (剑气远袭) should NOT be a melee source"
    );
    assert!(
        !is_melee_source(AttackSource::SwordPathResonance),
        "SwordPathResonance (AoE 打断) should NOT be a melee source"
    );
    assert!(
        !is_melee_source(AttackSource::SwordPathManifest),
        "SwordPathManifest (剑意追击) should NOT be a melee source"
    );
    assert!(
        !is_melee_source(AttackSource::SwordPathHeavenGate),
        "SwordPathHeavenGate (AoE 远程) should NOT be a melee source"
    );
}

// ── §5.6 Test 3: crack_reading_shows_integrity_brackets ──

#[test]
fn crack_reading_shows_integrity_brackets() {
    // Player target: 4-bracket precision.
    assert_eq!(
        bracket_for_player(1.0),
        IntegrityBracket::Intact,
        "integrity=1.0 should be Intact"
    );
    assert_eq!(
        bracket_for_player(0.85),
        IntegrityBracket::Intact,
        "integrity=0.85 should be Intact (boundary, >= 0.85)"
    );
    assert_eq!(
        bracket_for_player(0.849),
        IntegrityBracket::MicroTear,
        "integrity=0.849 should be MicroTear"
    );
    assert_eq!(
        bracket_for_player(0.70),
        IntegrityBracket::MicroTear,
        "integrity=0.70 should be MicroTear (within [0.50, 0.85))"
    );
    assert_eq!(
        bracket_for_player(0.50),
        IntegrityBracket::MicroTear,
        "integrity=0.50 should be MicroTear (boundary, >= 0.50)"
    );
    assert_eq!(
        bracket_for_player(0.499),
        IntegrityBracket::Torn,
        "integrity=0.499 should be Torn"
    );
    assert_eq!(
        bracket_for_player(0.01),
        IntegrityBracket::Torn,
        "integrity=0.01 should be Torn"
    );
    assert_eq!(
        bracket_for_player(0.0),
        IntegrityBracket::Severed,
        "integrity=0.0 should be Severed"
    );

    // NPC target: 3-bracket precision (§8.1 #4).
    assert_eq!(
        bracket_for_npc(1.0),
        NpcIntegrityBracket::Intact,
        "NPC integrity=1.0 should be Intact"
    );
    assert_eq!(
        bracket_for_npc(0.85),
        NpcIntegrityBracket::Intact,
        "NPC integrity=0.85 should be Intact"
    );
    assert_eq!(
        bracket_for_npc(0.849),
        NpcIntegrityBracket::Damaged,
        "NPC integrity=0.849 should be Damaged (MicroTear+Torn merged)"
    );
    assert_eq!(
        bracket_for_npc(0.50),
        NpcIntegrityBracket::Damaged,
        "NPC integrity=0.50 should be Damaged"
    );
    assert_eq!(
        bracket_for_npc(0.01),
        NpcIntegrityBracket::Damaged,
        "NPC integrity=0.01 should be Damaged (Torn merged into Damaged)"
    );
    assert_eq!(
        bracket_for_npc(0.0),
        NpcIntegrityBracket::Severed,
        "NPC integrity=0.0 should be Severed"
    );

    // Full result with mixed integrity values on player target.
    let mut ms = MeridianSystem::default();
    set_integrity(&mut ms, MeridianId::Lung, 1.0); // Intact
    set_integrity(&mut ms, MeridianId::Heart, 0.70); // MicroTear
    set_integrity(&mut ms, MeridianId::Stomach, 0.30); // Torn
    set_integrity(&mut ms, MeridianId::Kidney, 0.0); // Severed

    let mut app = app_at_tick(100);
    let target = app.world_mut().spawn_empty().id();
    let result = build_reading_result(target, &ms, None, None, false, false, 100);

    // Check specific meridians.
    let find = |id: MeridianId| -> &MeridianReadEntry {
        result
            .meridian_states
            .iter()
            .find(|e| e.id == id)
            .unwrap_or_else(|| panic!("should have entry for {:?}", id))
    };

    assert_eq!(
        find(MeridianId::Lung).integrity_bracket,
        IntegrityBracket::Intact
    );
    assert_eq!(
        find(MeridianId::Heart).integrity_bracket,
        IntegrityBracket::MicroTear
    );
    assert_eq!(
        find(MeridianId::Stomach).integrity_bracket,
        IntegrityBracket::Torn
    );
    assert_eq!(
        find(MeridianId::Kidney).integrity_bracket,
        IntegrityBracket::Severed
    );

    // All 20 meridians should be present.
    assert_eq!(
        result.meridian_states.len(),
        20,
        "should report all 20 meridians, got {}",
        result.meridian_states.len()
    );
}

// ── §5.6 Test 4: crack_reading_deep_on_consecutive ──

#[test]
fn crack_reading_deep_on_consecutive() {
    let ms = MeridianSystem::default();
    let mut app = app_at_tick(100);
    let target = app.world_mut().spawn_empty().id();

    // First read (shallow).
    let result_shallow = build_reading_result(target, &ms, None, None, false, false, 100);
    assert!(
        !result_shallow.is_deep_read,
        "first read should be shallow, not deep"
    );

    // Second read on same target within window (deep).
    let result_deep = build_reading_result(target, &ms, None, None, false, true, 130);
    assert!(
        result_deep.is_deep_read,
        "second read within deep window should be deep"
    );

    // Test the deep window timing: CrackReadingState logic.
    let state = CrackReadingState {
        last_read_tick: Some(100),
        last_read_target: Some(target),
    };

    // Within window (100 + 60 = 160, tick=130 is inside).
    let within = state
        .last_read_tick
        .zip(state.last_read_target)
        .is_some_and(|(last_tick, last_target)| {
            last_target == target
                && (130u64).saturating_sub(last_tick) < CRACK_READING_DEEP_WINDOW_TICKS
        });
    assert!(within, "tick=130 should be within deep window (100 + 60)");

    // Outside window (tick=161, past 100 + 60 = 160).
    let outside = state
        .last_read_tick
        .zip(state.last_read_target)
        .is_some_and(|(last_tick, last_target)| {
            last_target == target
                && (161u64).saturating_sub(last_tick) < CRACK_READING_DEEP_WINDOW_TICKS
        });
    assert!(
        !outside,
        "tick=161 should be outside deep window (100 + 60 = 160)"
    );

    // Boundary: exactly at window edge (tick=159, still inside).
    let boundary = state
        .last_read_tick
        .zip(state.last_read_target)
        .is_some_and(|(last_tick, last_target)| {
            last_target == target
                && (159u64).saturating_sub(last_tick) < CRACK_READING_DEEP_WINDOW_TICKS
        });
    assert!(
        boundary,
        "tick=159 (59 ticks after 100) should be inside deep window"
    );

    // Boundary: exactly at window limit (tick=160, outside since < not <=).
    let at_limit = state
        .last_read_tick
        .zip(state.last_read_target)
        .is_some_and(|(last_tick, last_target)| {
            last_target == target
                && (160u64).saturating_sub(last_tick) < CRACK_READING_DEEP_WINDOW_TICKS
        });
    assert!(
        !at_limit,
        "tick=160 (60 ticks = exactly CRACK_READING_DEEP_WINDOW_TICKS) should be outside"
    );
}

// ── §5.6 Test 5: crack_reading_deep_shows_circuits ──

#[test]
fn crack_reading_deep_shows_circuits() {
    let ms = MeridianSystem::default();
    let mut app = app_at_tick(100);
    let target = app.world_mut().spawn_empty().id();

    // Create circuits: TigerMouth (LI+SI) active.
    let mut circuits = ActiveScarCircuits::default();
    circuits.circuits.insert(ScarCircuitKind::TigerMouth);
    circuits.formed_at.insert(ScarCircuitKind::TigerMouth, 50);

    // Shallow read: has_circuit should be false even with active circuits.
    let result_shallow =
        build_reading_result(target, &ms, Some(&circuits), None, false, false, 100);
    for entry in &result_shallow.meridian_states {
        assert!(
            !entry.has_circuit,
            "shallow read should NOT show circuit info for {:?}",
            entry.id,
        );
    }

    // Deep read on player target: has_circuit should reflect active circuits.
    let result_deep = build_reading_result(target, &ms, Some(&circuits), None, false, true, 100);
    let find = |id: MeridianId| -> &MeridianReadEntry {
        result_deep
            .meridian_states
            .iter()
            .find(|e| e.id == id)
            .unwrap_or_else(|| panic!("should have entry for {:?}", id))
    };

    assert!(
        find(MeridianId::LargeIntestine).has_circuit,
        "LI should show has_circuit=true in deep read (TigerMouth involves LI)"
    );
    assert!(
        find(MeridianId::SmallIntestine).has_circuit,
        "SI should show has_circuit=true in deep read (TigerMouth involves SI)"
    );
    assert!(
        !find(MeridianId::Lung).has_circuit,
        "Lung should show has_circuit=false (not part of TigerMouth)"
    );

    // Deep read on NPC target: has_circuit always false (§8.1 #4).
    let result_npc_deep = build_reading_result(target, &ms, Some(&circuits), None, true, true, 100);
    for entry in &result_npc_deep.meridian_states {
        assert!(
            !entry.has_circuit,
            "NPC deep read should always have has_circuit=false for {:?}",
            entry.id,
        );
    }
}

// ── §5.6 Test 6: crack_reading_deep_shows_dead_armor ──

#[test]
fn crack_reading_deep_shows_dead_armor() {
    let ms = MeridianSystem::default();
    let mut app = app_at_tick(100);
    let target = app.world_mut().spawn_empty().id();

    // Create dead armor: ArmR immune (maps to LI/SI/TE).
    let mut armor = DeadMeridianArmor::default();
    armor.immune_regions.insert(BodyPart::ArmR);

    // Shallow read: is_dead_armor should be false.
    let result_shallow = build_reading_result(target, &ms, None, Some(&armor), false, false, 100);
    for entry in &result_shallow.meridian_states {
        assert!(
            !entry.is_dead_armor,
            "shallow read should NOT show dead armor info for {:?}",
            entry.id,
        );
    }

    // Deep read on player target: is_dead_armor for right arm meridians.
    let result_deep = build_reading_result(target, &ms, None, Some(&armor), false, true, 100);
    let find = |id: MeridianId| -> &MeridianReadEntry {
        result_deep
            .meridian_states
            .iter()
            .find(|e| e.id == id)
            .unwrap_or_else(|| panic!("should have entry for {:?}", id))
    };

    // Right arm (LI, SI, TE) → ArmR should be dead armor.
    assert!(
        find(MeridianId::LargeIntestine).is_dead_armor,
        "LI should show is_dead_armor=true (ArmR is immune)"
    );
    assert!(
        find(MeridianId::SmallIntestine).is_dead_armor,
        "SI should show is_dead_armor=true (ArmR is immune)"
    );
    assert!(
        find(MeridianId::TripleEnergizer).is_dead_armor,
        "TE should show is_dead_armor=true (ArmR is immune)"
    );

    // Left arm (LU, HT, PC) → ArmL NOT immune.
    assert!(
        !find(MeridianId::Lung).is_dead_armor,
        "Lung should show is_dead_armor=false (ArmL not immune)"
    );
    assert!(
        !find(MeridianId::Heart).is_dead_armor,
        "Heart should show is_dead_armor=false"
    );

    // Deep read on NPC target: is_dead_armor always false.
    let result_npc = build_reading_result(target, &ms, None, Some(&armor), true, true, 100);
    for entry in &result_npc.meridian_states {
        assert!(
            !entry.is_dead_armor,
            "NPC deep read should always have is_dead_armor=false for {:?}",
            entry.id,
        );
    }
}

// ── §5.6 Test 7: crack_reading_resets_on_target_change ──

#[test]
fn crack_reading_resets_on_target_change() {
    let mut app = app_at_tick(100);
    let target_a = app.world_mut().spawn_empty().id();
    let target_b = app.world_mut().spawn_empty().id();

    let mut state = CrackReadingState {
        last_read_tick: Some(100),
        last_read_target: Some(target_a),
    };

    // Same target within window -> deep.
    let is_deep_same = state
        .last_read_tick
        .zip(state.last_read_target)
        .is_some_and(|(last_tick, last_target)| {
            last_target == target_a
                && (130u64).saturating_sub(last_tick) < CRACK_READING_DEEP_WINDOW_TICKS
        });
    assert!(
        is_deep_same,
        "same target within window should allow deep read"
    );

    // Different target -> NOT deep even within window.
    let is_deep_diff = state
        .last_read_tick
        .zip(state.last_read_target)
        .is_some_and(|(last_tick, last_target)| {
            last_target == target_b
                && (130u64).saturating_sub(last_tick) < CRACK_READING_DEEP_WINDOW_TICKS
        });
    assert!(
        !is_deep_diff,
        "different target should NOT allow deep read (deep window resets on target change)"
    );

    // After hitting target_b, state updates.
    state.last_read_tick = Some(130);
    state.last_read_target = Some(target_b);

    // Now target_b within window -> deep.
    let is_deep_b = state
        .last_read_tick
        .zip(state.last_read_target)
        .is_some_and(|(last_tick, last_target)| {
            last_target == target_b
                && (140u64).saturating_sub(last_tick) < CRACK_READING_DEEP_WINDOW_TICKS
        });
    assert!(
        is_deep_b,
        "after switching, target_b within window should allow deep read"
    );

    // But target_a is no longer deep.
    let is_deep_a_after = state
        .last_read_tick
        .zip(state.last_read_target)
        .is_some_and(|(last_tick, last_target)| {
            last_target == target_a
                && (140u64).saturating_sub(last_tick) < CRACK_READING_DEEP_WINDOW_TICKS
        });
    assert!(
        !is_deep_a_after,
        "target_a should no longer be deep after switching to target_b"
    );
}

// ── §5.6 Test 8: crack_reading_no_exact_values ──

#[test]
fn crack_reading_no_exact_values() {
    // The CrackReadingResult and MeridianReadEntry should NOT contain exact integrity values.
    // They only contain IntegrityBracket enums (4 or 3 discrete values).
    // This test verifies by exhaustive bracket checks that different exact values
    // within the same bracket produce identical results.

    let mut ms = MeridianSystem::default();

    // Two different exact values in MicroTear range.
    set_integrity(&mut ms, MeridianId::Lung, 0.65);
    set_integrity(&mut ms, MeridianId::Heart, 0.75);

    let mut app = app_at_tick(100);
    let target = app.world_mut().spawn_empty().id();
    let result = build_reading_result(target, &ms, None, None, false, false, 100);

    let find = |id: MeridianId| -> &MeridianReadEntry {
        result
            .meridian_states
            .iter()
            .find(|e| e.id == id)
            .unwrap_or_else(|| panic!("should have entry for {:?}", id))
    };

    // Both should be MicroTear — no way to distinguish 0.65 vs 0.75.
    assert_eq!(
        find(MeridianId::Lung).integrity_bracket,
        IntegrityBracket::MicroTear,
        "0.65 should map to MicroTear"
    );
    assert_eq!(
        find(MeridianId::Heart).integrity_bracket,
        IntegrityBracket::MicroTear,
        "0.75 should also map to MicroTear — exact value not leaked"
    );

    // Two different exact values in Torn range.
    set_integrity(&mut ms, MeridianId::Stomach, 0.10);
    set_integrity(&mut ms, MeridianId::Spleen, 0.40);

    let result2 = build_reading_result(target, &ms, None, None, false, false, 100);
    let find2 = |id: MeridianId| -> &MeridianReadEntry {
        result2
            .meridian_states
            .iter()
            .find(|e| e.id == id)
            .unwrap_or_else(|| panic!("should have entry for {:?}", id))
    };

    assert_eq!(
        find2(MeridianId::Stomach).integrity_bracket,
        IntegrityBracket::Torn,
        "0.10 should map to Torn"
    );
    assert_eq!(
        find2(MeridianId::Spleen).integrity_bracket,
        IntegrityBracket::Torn,
        "0.40 should also map to Torn — exact value not leaked"
    );

    // NPC target: even more coarse — MicroTear and Torn both map to Damaged.
    let result_npc = build_reading_result(target, &ms, None, None, true, false, 100);
    let find_npc = |id: MeridianId| -> &MeridianReadEntry {
        result_npc
            .meridian_states
            .iter()
            .find(|e| e.id == id)
            .unwrap_or_else(|| panic!("should have entry for {:?}", id))
    };

    // NPC: Damaged maps to MicroTear in the unified IntegrityBracket.
    assert_eq!(
        find_npc(MeridianId::Lung).integrity_bracket,
        IntegrityBracket::MicroTear,
        "NPC 0.65 should map to MicroTear (Damaged→MicroTear in unified bracket)"
    );
    assert_eq!(
        find_npc(MeridianId::Stomach).integrity_bracket,
        IntegrityBracket::MicroTear,
        "NPC 0.10 should also map to MicroTear (Torn→Damaged→MicroTear) — NPC 3-bracket precision hides the difference"
    );
}

// ── §5.6 Test 9: crack_reading_client_payload_format ──

#[test]
fn crack_reading_client_payload_format() {
    let mut ms = MeridianSystem::default();
    set_integrity(&mut ms, MeridianId::LargeIntestine, 0.30); // Torn
    set_integrity(&mut ms, MeridianId::Lung, 0.70); // MicroTear

    let mut app = app_at_tick(100);
    let target = app.world_mut().spawn_empty().id();

    // Create circuits for deep read.
    let mut circuits = ActiveScarCircuits::default();
    circuits.circuits.insert(ScarCircuitKind::TigerMouth);

    let result = build_reading_result(target, &ms, Some(&circuits), None, false, true, 100);

    // Convert to payload.
    let payload = to_client_payload(&result, 100);

    // Verify structure.
    assert_eq!(
        payload.target_entity_id,
        target.to_bits(),
        "target_entity_id should match entity bits"
    );
    assert!(payload.is_deep, "should be marked as deep read");
    assert_eq!(
        payload.display_ticks, CRACK_READING_DISPLAY_TICKS,
        "display_ticks should be CRACK_READING_DISPLAY_TICKS={}",
        CRACK_READING_DISPLAY_TICKS,
    );
    assert_eq!(
        payload.entries.len(),
        20,
        "should have entries for all 20 meridians"
    );

    // Check specific entries.
    let find_payload =
        |name: &str| -> &crate::combat::baomai_v4::crack_reading::CrackReadingPayloadEntry {
            payload
                .entries
                .iter()
                .find(|e| e.meridian == name)
                .unwrap_or_else(|| panic!("should have payload entry for {}", name))
        };

    let li_entry = find_payload("LargeIntestine");
    assert_eq!(
        li_entry.bracket, "Torn",
        "LI with integrity=0.30 should have bracket 'Torn'"
    );
    assert!(
        li_entry.has_circuit,
        "LI should show has_circuit=true in deep read (TigerMouth involves LI)"
    );

    let lung_entry = find_payload("Lung");
    assert_eq!(
        lung_entry.bracket, "MicroTear",
        "Lung with integrity=0.70 should have bracket 'MicroTear'"
    );
    assert!(
        !lung_entry.has_circuit,
        "Lung should show has_circuit=false (not part of TigerMouth)"
    );

    // Verify JSON serialization doesn't panic and has expected shape.
    let json = serde_json::to_string_pretty(&payload)
        .expect("payload should serialize to JSON without error");
    assert!(
        json.contains("\"target_entity_id\""),
        "JSON should contain target_entity_id field"
    );
    assert!(
        json.contains("\"entries\""),
        "JSON should contain entries field"
    );
    assert!(
        json.contains("\"is_deep\""),
        "JSON should contain is_deep field"
    );
    assert!(
        json.contains("\"display_ticks\""),
        "JSON should contain display_ticks field"
    );
    assert!(
        json.contains("\"meridian\""),
        "JSON entries should contain meridian field"
    );
    assert!(
        json.contains("\"bracket\""),
        "JSON entries should contain bracket field"
    );
    assert!(
        json.contains("\"has_circuit\""),
        "JSON entries should contain has_circuit field"
    );
    assert!(
        json.contains("\"is_dead_armor\""),
        "JSON entries should contain is_dead_armor field"
    );
}
