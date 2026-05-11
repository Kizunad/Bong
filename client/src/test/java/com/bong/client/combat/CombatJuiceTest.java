package com.bong.client.combat;

import com.bong.client.combat.juice.CameraShakeController;
import com.bong.client.combat.juice.CombatJuiceCalibration;
import com.bong.client.combat.juice.CombatJuiceEvent;
import com.bong.client.combat.juice.CombatJuiceProfile;
import com.bong.client.combat.juice.CombatJuiceSystem;
import com.bong.client.combat.juice.CombatJuiceTier;
import com.bong.client.combat.juice.CombatSchool;
import com.bong.client.combat.juice.EntityTintController;
import com.bong.client.combat.juice.HitStopController;
import com.bong.client.combat.juice.KillJuiceController;
import com.bong.client.combat.juice.ParryDodgeJuicePlanner;
import com.bong.client.combat.juice.WoundWorldVisualPlanner;
import com.bong.client.combat.store.StatusEffectStore;
import com.bong.client.combat.store.WoundsStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

class CombatJuiceTest {
    @BeforeEach
    @AfterEach
    void resetState() {
        CombatJuiceSystem.resetForTests();
    }

    @Test
    void juice_profile_selects_by_tier() {
        CombatJuiceProfile critical = CombatJuiceProfile.select(CombatSchool.BAOMAI, CombatJuiceTier.CRITICAL);
        assertEquals(10, critical.hitStopTicks());
        assertEquals(0.90f, critical.shakeIntensity(), 0.001f);
        assertEquals(0xFFB87333, critical.qiColorArgb());

        CombatJuiceProfile poison = CombatJuiceProfile.select(CombatSchool.DUGU, CombatJuiceTier.CRITICAL);
        assertEquals(2, poison.hitStopTicks(), "dugu critical should stay low-impact but visible");
        assertEquals(45, poison.tintDurationTicks(), "dugu uses long invasive tint instead of impact shake");

        assertEquals(21, CombatJuiceProfile.profiles().size(), "7 schools x 3 tiers must be configured");
    }

    @Test
    void hit_stop_freezes_entity() {
        CombatJuiceProfile profile = CombatJuiceProfile.select(CombatSchool.BAOMAI, CombatJuiceTier.HEAVY);
        HitStopController.request("attacker", "target", profile, 1_000L);

        assertEquals(6, HitStopController.remainingTicks("target", 1_000L), "expected defender to receive full heavy hit-stop budget because target was hit, actual remaining ticks differed");
        assertEquals(3, HitStopController.remainingTicks("attacker", 1_000L), "expected attacker to receive half heavy hit-stop budget because local swing recovery is shorter, actual remaining ticks differed");
        assertTrue(HitStopController.isFrozen("target", 1_100L), "expected target to remain frozen 100ms into a 6 tick freeze because duration is 300ms, actual was unfrozen");
    }

    @Test
    void hit_stop_attacker_ticks_floor_half_budget() {
        CombatJuiceProfile profile = CombatJuiceProfile.select(CombatSchool.GENERIC, CombatJuiceTier.HEAVY);
        HitStopController.request("attacker", "target", profile, 1_000L);

        assertEquals(5, HitStopController.remainingTicks("target", 1_000L), "expected generic heavy defender freeze to use the full 5 tick profile budget, actual remaining ticks differed");
        assertEquals(2, HitStopController.remainingTicks("attacker", 1_000L), "expected attacker freeze to floor half of 5 ticks to 2 because attacker recovery must not exceed design budget, actual remaining ticks differed");
    }

    @Test
    void shake_direction_perpendicular() {
        double[] normal = CameraShakeController.perpendicular(1.0, 0.0, false);
        assertEquals(0.0, normal[0], 0.0001);
        assertEquals(1.0, normal[1], 0.0001);

        double[] reverse = CameraShakeController.perpendicular(1.0, 0.0, true);
        assertEquals(0.0, reverse[0], 0.0001);
        assertEquals(-1.0, reverse[1], 0.0001);
    }

    @Test
    void shake_decays_linearly() {
        CameraShakeController.Shake shake = new CameraShakeController.Shake(1.0f, 10, 1.0, 0.0, false, 1_000L);
        assertEquals(1.0, shake.remainingRatioAt(1_000L), 0.0001);
        assertEquals(0.5, shake.remainingRatioAt(1_250L), 0.0001);
        assertEquals(0.0, shake.remainingRatioAt(1_500L), 0.0001);
    }

    @Test
    void qi_collision_selects_school_color() {
        CombatJuiceEvent event = new CombatJuiceEvent(
            CombatJuiceEvent.Kind.QI_COLLISION,
            CombatSchool.ZHENMAI,
            CombatJuiceTier.LIGHT,
            "attacker",
            "target",
            "",
            "",
            0.0,
            1.0,
            false,
            2_000L
        );
        CombatJuiceSystem.LastCommand command = CombatJuiceSystem.accept(event, 2_000L);

        assertEquals(0xFF4682B4, command.profile().qiColorArgb());
        assertEquals(0xFF4682B4, command.tint().argb());
    }

    @Test
    void entity_tint_lerp_back() {
        EntityTintController.Tint tint =
            EntityTintController.trigger("target", CombatJuiceProfile.select(CombatSchool.BAOMAI, CombatJuiceTier.LIGHT), 1_000L);

        assertEquals(0.4f, tint.alphaAt(1_000L), 0.001f);
        assertEquals(0.2f, tint.alphaAt(1_150L), 0.001f);
        assertEquals(0.0f, tint.alphaAt(1_300L), 0.001f);
    }

    @Test
    void full_charge_max_juice() {
        CombatJuiceEvent event = CombatJuiceEvent.hit(
            CombatSchool.BAOMAI,
            CombatJuiceTier.LIGHT,
            "attacker",
            "target",
            0.0,
            1.0,
            3_000L
        );
        event = new CombatJuiceEvent(
            CombatJuiceEvent.Kind.FULL_CHARGE,
            event.school(),
            event.tier(),
            event.attackerUuid(),
            event.targetUuid(),
            "",
            "",
            event.directionX(),
            event.directionZ(),
            false,
            event.receivedAtMs()
        );
        CombatJuiceSystem.LastCommand command = CombatJuiceSystem.accept(event, 3_000L);

        assertEquals(CombatJuiceTier.CRITICAL, command.profile().tier());
        assertEquals(10, HitStopController.remainingTicks("target", 3_000L));
        assertTrue(command.overlay().activeAt(3_000L));
    }

    @Test
    void full_charge_alias_infers_heavy_tier_without_explicit_tier() {
        assertEquals(
            CombatJuiceTier.HEAVY,
            CombatJuiceTier.fromCombatEvent("full_charge", 1.0, null),
            "expected full_charge alias to infer HEAVY because fromWire(full_charge) maps to the same tier, actual tier differed"
        );
    }

    @Test
    void accept_null_event_returns_empty_command() {
        CombatJuiceSystem.LastCommand command = CombatJuiceSystem.accept(null, 1_000L);

        assertNull(command.event(), "expected null combat event to produce an empty command because invalid input must be ignored safely");
        assertFalse(command.overlay().activeAt(1_000L), "expected null combat event to have no active overlay because no visual branch ran");
    }

    @Test
    void accept_clears_expired_overlay_before_next_command_snapshot() {
        CombatJuiceEvent overload = new CombatJuiceEvent(
            CombatJuiceEvent.Kind.OVERLOAD,
            CombatSchool.BAOMAI,
            CombatJuiceTier.LIGHT,
            "attacker",
            "target",
            "",
            "",
            0.0,
            1.0,
            false,
            1_000L
        );
        CombatJuiceSystem.LastCommand overloadCommand = CombatJuiceSystem.accept(overload, 1_000L);
        assertTrue(overloadCommand.overlay().activeAt(1_000L), "expected overload command to carry active overlay because overload creates a 10 tick vignette");

        CombatJuiceEvent hit = CombatJuiceEvent.hit(CombatSchool.BAOMAI, CombatJuiceTier.LIGHT, "attacker", "target", 0.0, 1.0, 1_501L);
        CombatJuiceSystem.LastCommand hitCommand = CombatJuiceSystem.accept(hit, 1_501L);

        assertFalse(hitCommand.overlay().activeAt(1_501L), "expected later hit command to drop expired overload overlay because command snapshots must not carry stale overlays");
        assertEquals(CombatJuiceEvent.Kind.HIT, hitCommand.event().kind(), "expected the post-overlay command to still process the new hit event, actual kind differed");
    }

    @Test
    void overload_red_freeze() {
        CombatJuiceEvent event = new CombatJuiceEvent(
            CombatJuiceEvent.Kind.OVERLOAD,
            CombatSchool.BAOMAI,
            CombatJuiceTier.LIGHT,
            "attacker",
            "target",
            "",
            "",
            0.0,
            1.0,
            false,
            4_000L
        );
        CombatJuiceSystem.LastCommand command = CombatJuiceSystem.accept(event, 4_000L);

        assertEquals(10, HitStopController.remainingTicks("target", 4_000L));
        assertTrue(command.overlay().vignette());
        assertEquals(0x00FF2020, command.overlay().argb() & 0x00FFFFFF);
    }

    @Test
    void parry_pushback_both_sides() {
        CombatJuiceEvent event = new CombatJuiceEvent(
            CombatJuiceEvent.Kind.PARRY,
            CombatSchool.ZHENMAI,
            CombatJuiceTier.LIGHT,
            "attacker",
            "defender",
            "",
            "",
            0.0,
            1.0,
            false,
            5_000L
        );
        ParryDodgeJuicePlanner.ParryPlan plan = ParryDodgeJuicePlanner.parry(event, false);

        assertEquals("attacker", plan.attackerPushback().entityUuid());
        assertEquals(-0.3, plan.attackerPushback().velocityZ(), 0.0001);
        assertEquals("defender", plan.defenderPushback().entityUuid());
        assertEquals(0.3, plan.defenderPushback().velocityZ(), 0.0001);
    }

    @Test
    void perfect_parry_white_flash() {
        CombatJuiceEvent event = new CombatJuiceEvent(
            CombatJuiceEvent.Kind.PERFECT_PARRY,
            CombatSchool.ZHENMAI,
            CombatJuiceTier.LIGHT,
            "attacker",
            "defender",
            "",
            "",
            1.0,
            0.0,
            false,
            5_000L
        );
        CombatJuiceSystem.LastCommand command = CombatJuiceSystem.accept(event, 5_000L);

        assertNotNull(command.parry());
        assertTrue(command.parry().perfect());
        assertEquals(0x00FFFFFF, command.overlay().argb() & 0x00FFFFFF);
    }

    @Test
    void dodge_ghost_entity_fades() {
        ParryDodgeJuicePlanner.DodgeGhost ghost = ParryDodgeJuicePlanner.dodge("player", 0xFFCCAA88, 1_000L);

        assertEquals(0.4f, ghost.alphaAt(1_000L), 0.001f);
        assertTrue(ghost.alphaAt(1_250L) < ghost.alphaAt(1_000L));
        assertEquals(0.0f, ghost.alphaAt(1_500L), 0.001f);
    }

    @Test
    void fracture_tilts_limb() {
        List<WoundWorldVisualPlanner.WoundCommand> commands = WoundWorldVisualPlanner.plan(
            List.of(new WoundsStore.Wound("left_arm", "bone_fracture", 0.7f, WoundsStore.HealingState.BLEEDING, 0f, false, 0L)),
            List.of(),
            false
        );

        assertEquals(1, commands.size());
        assertEquals(5.0f, commands.get(0).limbTiltDegrees(), 0.001f);
    }

    @Test
    void severed_drip_particle() {
        List<WoundWorldVisualPlanner.WoundCommand> commands = WoundWorldVisualPlanner.plan(
            List.of(new WoundsStore.Wound("right_arm", "limb_severed", 0.9f, WoundsStore.HealingState.BLEEDING, 0f, false, 0L)),
            List.of(),
            false
        );

        assertTrue(commands.get(0).dripParticle(), "expected explicit limb_severed wound to emit drip particles because only amputation-type wounds should look severed, actual command did not drip");
    }

    @Test
    void high_severity_cut_does_not_trigger_severed_visuals() {
        List<WoundWorldVisualPlanner.WoundCommand> commands = WoundWorldVisualPlanner.plan(
            List.of(new WoundsStore.Wound("right_arm", "cut", 0.95f, WoundsStore.HealingState.BLEEDING, 0f, false, 0L)),
            List.of(),
            false
        );

        assertTrue(commands.isEmpty(), "expected high-severity non-amputation cut to avoid severed visuals because severity alone is not an amputation signal, actual commands=" + commands);
    }

    @Test
    void wound_visual_planner_handles_blank_network_fields() {
        List<WoundWorldVisualPlanner.WoundCommand> commands = WoundWorldVisualPlanner.plan(
            List.of(new WoundsStore.Wound(null, null, 0.95f, WoundsStore.HealingState.BLEEDING, 0f, false, 0L)),
            List.of(new StatusEffectStore.Effect(null, null, null, 1, 1_000L, 0, null, 0)),
            false
        );

        assertTrue(commands.isEmpty(), "expected blank wound/effect ids to be ignored because missing optional network fields must not create false visuals, actual commands=" + commands);
    }

    @Test
    void contamination_meridian_glow() {
        List<WoundWorldVisualPlanner.WoundCommand> commands = WoundWorldVisualPlanner.plan(
            List.of(new WoundsStore.Wound("chest", "qi_wound", 0.2f, WoundsStore.HealingState.BLEEDING, 0.8f, false, 0L)),
            List.of(),
            false
        );

        assertTrue(commands.stream().anyMatch(WoundWorldVisualPlanner.WoundCommand::meridianGlow));
        assertTrue(commands.stream().anyMatch(WoundWorldVisualPlanner.WoundCommand::coughAudio));
    }

    @Test
    void exhausted_stumble_interval() {
        List<WoundWorldVisualPlanner.WoundCommand> commands = WoundWorldVisualPlanner.plan(List.of(), List.of(), true);

        assertTrue(commands.stream().anyMatch(WoundWorldVisualPlanner.WoundCommand::exhaustedStumble));
    }

    @Test
    void kill_slowmo_only_for_killer() {
        CombatJuiceProfile profile = CombatJuiceProfile.select(CombatSchool.BAOMAI, CombatJuiceTier.CRITICAL);
        CombatJuiceEvent remoteKill = new CombatJuiceEvent(
            CombatJuiceEvent.Kind.KILL,
            CombatSchool.BAOMAI,
            CombatJuiceTier.CRITICAL,
            "attacker",
            "target",
            "someone_else",
            "rat",
            0.0,
            1.0,
            false,
            1_000L
        );

        assertFalse(KillJuiceController.trigger(remoteKill, profile, 1_000L).activeAt(1_000L), "expected remote kill to suppress slowmo because local player is not attacker, actual state was active");

        CombatJuiceEvent unknownLocalKill = new CombatJuiceEvent(
            CombatJuiceEvent.Kind.KILL,
            CombatSchool.BAOMAI,
            CombatJuiceTier.CRITICAL,
            "attacker",
            "target",
            "",
            "rat",
            0.0,
            1.0,
            false,
            1_000L
        );

        assertFalse(KillJuiceController.trigger(unknownLocalKill, profile, 1_000L).activeAt(1_000L), "expected blank local uuid to suppress kill slowmo because unknown identity must not count as local attacker, actual state was active");

        CombatJuiceEvent localKill = new CombatJuiceEvent(
            CombatJuiceEvent.Kind.KILL,
            CombatSchool.BAOMAI,
            CombatJuiceTier.CRITICAL,
            "attacker",
            "target",
            "attacker",
            "rat",
            0.0,
            1.0,
            false,
            1_000L
        );

        assertTrue(KillJuiceController.trigger(localKill, profile, 1_000L).activeAt(1_000L), "expected local attacker kill to trigger slowmo because local uuid matches attacker, actual state was inactive");
        assertTrue(KillJuiceController.fovDelta(1_000L) < 0.0, "expected local kill slowmo to push FOV negative because kill juice adds impact zoom, actual delta=" + KillJuiceController.fovDelta(1_000L));
    }

    @Test
    void rare_drop_golden_pillar() {
        CombatJuiceEvent localKill = new CombatJuiceEvent(
            CombatJuiceEvent.Kind.KILL,
            CombatSchool.ZHENFA,
            CombatJuiceTier.CRITICAL,
            "attacker",
            "target",
            "attacker",
            "elite",
            0.0,
            1.0,
            true,
            1_000L
        );

        KillJuiceController.KillState state =
            KillJuiceController.trigger(localKill, CombatJuiceProfile.select(CombatSchool.ZHENFA, CombatJuiceTier.CRITICAL), 1_000L);

        assertTrue(state.rareDrop());
    }

    @Test
    void multi_kill_counter_stacks() {
        CombatJuiceProfile profile = CombatJuiceProfile.select(CombatSchool.BAOMAI, CombatJuiceTier.CRITICAL);
        CombatJuiceEvent kill = new CombatJuiceEvent(
            CombatJuiceEvent.Kind.KILL,
            CombatSchool.BAOMAI,
            CombatJuiceTier.CRITICAL,
            "attacker",
            "target",
            "attacker",
            "target",
            0.0,
            1.0,
            false,
            1_000L
        );

        KillJuiceController.trigger(kill, profile, 1_000L);
        KillJuiceController.trigger(kill, profile, 4_000L);

        assertEquals(2, KillJuiceController.multiKill().count(), "expected second kill inside 5s window to stack multi-kill count to 2, actual count differed");
        assertEquals(1.2, KillJuiceController.multiKill().shakeMultiplier(), 0.0001, "expected second kill to raise shake multiplier to 1.2, actual multiplier differed");
        assertEquals(1.1, KillJuiceController.multiKill().pitchMultiplier(), 0.0001, "expected second kill to raise pitch multiplier to 1.1, actual multiplier differed");
    }

    @Test
    void multi_kill_counter_expires_after_window() {
        CombatJuiceProfile profile = CombatJuiceProfile.select(CombatSchool.BAOMAI, CombatJuiceTier.CRITICAL);
        CombatJuiceEvent kill = new CombatJuiceEvent(
            CombatJuiceEvent.Kind.KILL,
            CombatSchool.BAOMAI,
            CombatJuiceTier.CRITICAL,
            "attacker",
            "target",
            "attacker",
            "target",
            0.0,
            1.0,
            false,
            1_000L
        );

        KillJuiceController.trigger(kill, profile, 1_000L);
        KillJuiceController.trigger(kill, profile, 6_001L);

        assertEquals(1, KillJuiceController.multiKill().count(), "expected kill after the 5s window to reset multi-kill count because previous chain expired, actual count differed");
    }

    @Test
    void pvp_calibration_matrix_covers_49_pairings() {
        List<CombatJuiceCalibration.PvpPairing> pairings = CombatJuiceCalibration.pvpPairings();

        assertEquals(49, pairings.size());
        assertFalse(pairings.stream().anyMatch(CombatJuiceCalibration.PvpPairing::inputLagRisk));
        assertTrue(pairings.stream().anyMatch(CombatJuiceCalibration.PvpPairing::sameQiColor));
    }

    @Test
    void mixed_battle_budget_stays_above_30fps_floor() {
        CombatJuiceCalibration.PerformanceBudget budget = CombatJuiceCalibration.mixedBattleBudget(10, 10);

        assertEquals(40, budget.maxConcurrentJuiceEvents(), "expected 5v5 to budget 40 concurrent juice events because budget is 4 events per player across 10 players, actual event budget differed");
        assertTrue(budget.passesPlanFloor(), "expected 5v5 10min budget to satisfy 30fps floor because plan requires that scenario, actual budget=" + budget);
    }

    @Test
    void mixed_battle_budget_clamps_large_inputs_without_overflow() {
        CombatJuiceCalibration.PerformanceBudget budget = CombatJuiceCalibration.mixedBattleBudget(Integer.MAX_VALUE, 10);

        assertEquals(Integer.MAX_VALUE, budget.maxConcurrentJuiceEvents(), "expected huge player count to clamp maxConcurrentJuiceEvents to Integer.MAX_VALUE because int budget field cannot represent larger values, actual budget differed");
        assertEquals(30, budget.estimatedFpsFloor(), "expected huge event count to clamp estimated FPS floor to 30 instead of overflowing below the minimum, actual floor differed");
    }
}
