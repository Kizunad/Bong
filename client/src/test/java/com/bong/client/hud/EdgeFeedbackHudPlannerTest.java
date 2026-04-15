package com.bong.client.hud;

import com.bong.client.combat.CastOutcome;
import com.bong.client.combat.CastState;
import com.bong.client.combat.CombatHudState;
import com.bong.client.combat.DefenseWindowState;
import com.bong.client.combat.DerivedAttrFlags;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class EdgeFeedbackHudPlannerTest {

    @Test
    void healthyPlayerHasNoPulses() {
        CombatHudState full = CombatHudState.create(1.0f, 1.0f, 1.0f, DerivedAttrFlags.none());
        List<HudRenderCommand> cmds = EdgeFeedbackHudPlanner.buildCommands(
            full, DefenseWindowState.idle(), CastState.idle(), 0L, 1920, 1080);
        assertTrue(cmds.isEmpty(), "no pulses at 100% HP: " + cmds);
    }

    @Test
    void lowHpEmitsPulseVignette() {
        CombatHudState low = CombatHudState.create(0.25f, 0.5f, 0.5f, DerivedAttrFlags.none());
        List<HudRenderCommand> cmds = EdgeFeedbackHudPlanner.buildCommands(
            low, DefenseWindowState.idle(), CastState.idle(), 0L, 1920, 1080);
        assertEquals(1, cmds.size());
        assertTrue(cmds.get(0).isEdgeVignette());
    }

    @Test
    void criticalHpEmitsStrongerPulse() {
        CombatHudState crit = CombatHudState.create(0.05f, 0.5f, 0.5f, DerivedAttrFlags.none());
        List<HudRenderCommand> cmds = EdgeFeedbackHudPlanner.buildCommands(
            crit, DefenseWindowState.idle(), CastState.idle(), 200L, 1920, 1080);
        assertEquals(1, cmds.size());
        assertTrue(cmds.get(0).isEdgeVignette());
    }

    @Test
    void defenseWindowAddsFourEdgeFlash() {
        CombatHudState full = CombatHudState.create(1.0f, 1.0f, 1.0f, DerivedAttrFlags.none());
        DefenseWindowState dw = DefenseWindowState.active(200, 0L, 200L);
        List<HudRenderCommand> cmds = EdgeFeedbackHudPlanner.buildCommands(
            full, dw, CastState.idle(), 50L, 1920, 1080);
        assertEquals(4, cmds.size(), "4 edge rects for the flash");
        for (HudRenderCommand c : cmds) {
            assertTrue(c.isRect());
        }
    }

    @Test
    void expiredDefenseWindowDoesNotFlash() {
        CombatHudState full = CombatHudState.create(1.0f, 1.0f, 1.0f, DerivedAttrFlags.none());
        DefenseWindowState dw = DefenseWindowState.active(200, 0L, 200L);
        List<HudRenderCommand> cmds = EdgeFeedbackHudPlanner.buildCommands(
            full, dw, CastState.idle(), 500L, 1920, 1080);
        assertTrue(cmds.isEmpty());
    }

    @Test
    void phasingFlagEmitsFullScreenTint() {
        CombatHudState phasing = CombatHudState.create(1.0f, 1.0f, 1.0f, DerivedAttrFlags.of(false, true, false));
        List<HudRenderCommand> cmds = EdgeFeedbackHudPlanner.buildCommands(
            phasing, DefenseWindowState.idle(), CastState.idle(), 0L, 1920, 1080);
        assertFalse(cmds.isEmpty());
        assertTrue(cmds.get(0).isScreenTint());
    }

    @Test
    void tribulationFlagEmitsVignette() {
        CombatHudState trib = CombatHudState.create(1.0f, 1.0f, 1.0f, DerivedAttrFlags.of(false, false, true));
        List<HudRenderCommand> cmds = EdgeFeedbackHudPlanner.buildCommands(
            trib, DefenseWindowState.idle(), CastState.idle(), 0L, 1920, 1080);
        // tribulation vignette + no HP issue
        assertTrue(cmds.stream().anyMatch(HudRenderCommand::isEdgeVignette));
    }

    @Test
    void castInterruptEmitsFlashForFadeOutWindow() {
        CombatHudState full = CombatHudState.create(1.0f, 1.0f, 1.0f, DerivedAttrFlags.none());
        CastState cast = CastState.casting(0, 1000, 0L).transitionToInterrupt(CastOutcome.INTERRUPT_MOVEMENT, 100L);
        List<HudRenderCommand> recent = EdgeFeedbackHudPlanner.buildCommands(
            full, DefenseWindowState.idle(), cast, 150L, 1920, 1080);
        assertEquals(4, recent.size(), "edge flash rects during 0.3s fade");

        List<HudRenderCommand> later = EdgeFeedbackHudPlanner.buildCommands(
            full, DefenseWindowState.idle(), cast, 500L, 1920, 1080);
        assertTrue(later.isEmpty(), "no flash after 0.3s fade");
    }
}
