package com.bong.client.hud;

import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

class SwordPathHudPlannerTest {

    @Test
    void inactiveStateReturnsEmpty() {
        List<HudRenderCommand> cmds = SwordPathHudPlanner.buildCommands(
            SwordBondHudState.INACTIVE, System.currentTimeMillis(), 800, 600
        );
        assertTrue(cmds.isEmpty(), "inactive bond should produce no HUD commands");
    }

    @Test
    void nullStateReturnsEmpty() {
        List<HudRenderCommand> cmds = SwordPathHudPlanner.buildCommands(
            null, System.currentTimeMillis(), 800, 600
        );
        assertTrue(cmds.isEmpty(), "null state should produce no HUD commands");
    }

    @Test
    void zeroScreenReturnsEmpty() {
        SwordBondHudState state = new SwordBondHudState(true, 3, "凝脉", 0.5f, 0.8f, false);
        List<HudRenderCommand> cmds = SwordPathHudPlanner.buildCommands(state, 0, 0, 0);
        assertTrue(cmds.isEmpty(), "zero-size screen should produce no HUD commands");
    }

    @Test
    void activeStateProducesCommands() {
        SwordBondHudState state = new SwordBondHudState(true, 4, "固元", 0.6f, 0.7f, false);
        List<HudRenderCommand> cmds = SwordPathHudPlanner.buildCommands(
            state, System.currentTimeMillis(), 800, 600
        );
        assertFalse(cmds.isEmpty(), "active bond should produce HUD commands");
    }

    @Test
    void heavenGateReadyAddsExtraCommand() {
        SwordBondHudState noReady = new SwordBondHudState(true, 6, "化虚", 0.9f, 0.95f, false);
        SwordBondHudState ready = new SwordBondHudState(true, 6, "化虚", 0.9f, 0.95f, true);
        long now = System.currentTimeMillis();

        List<HudRenderCommand> cmdsNo = SwordPathHudPlanner.buildCommands(noReady, now, 800, 600);
        List<HudRenderCommand> cmdsYes = SwordPathHudPlanner.buildCommands(ready, now, 800, 600);

        assertTrue(
            cmdsYes.size() > cmdsNo.size(),
            "heaven gate ready should add extra command: " + cmdsYes.size() + " > " + cmdsNo.size()
        );
    }

    @Test
    void gradeColorBoundsCheck() {
        assertEquals(SwordPathHudPlanner.gradeColor(0), SwordPathHudPlanner.gradeColor(-1),
            "negative grade should clamp to grade 0 color");
        assertEquals(SwordPathHudPlanner.gradeColor(6), SwordPathHudPlanner.gradeColor(99),
            "over-max grade should clamp to highest grade color");
    }

    @Test
    void allGradesHaveDistinctColors() {
        for (int i = 0; i < 7; i++) {
            for (int j = i + 1; j < 7; j++) {
                assertNotEquals(
                    SwordPathHudPlanner.gradeColor(i),
                    SwordPathHudPlanner.gradeColor(j),
                    "grade " + i + " and " + j + " should have distinct colors"
                );
            }
        }
    }

    @Test
    void stateClamp01() {
        SwordBondHudState state = new SwordBondHudState(true, 3, "凝脉", 2.0f, -0.5f, false);
        assertEquals(1.0f, state.storedQiRatio(), 1e-6, "storedQiRatio should clamp to 1.0");
        assertEquals(0.0f, state.bondStrength(), 1e-6, "bondStrength should clamp to 0.0");
    }

    @Test
    void stateNanClamp() {
        SwordBondHudState state = new SwordBondHudState(true, 0, "", Float.NaN, Float.NaN, false);
        assertEquals(0.0f, state.storedQiRatio(), "NaN storedQiRatio should clamp to 0");
        assertEquals(0.0f, state.bondStrength(), "NaN bondStrength should clamp to 0");
    }

    @Test
    void storeReplaceAndSnapshot() {
        SwordBondHudStateStore.clear();
        assertFalse(SwordBondHudStateStore.snapshot().active(), "should start inactive");

        SwordBondHudState active = new SwordBondHudState(true, 2, "引气", 0.3f, 0.5f, false);
        SwordBondHudStateStore.replace(active);
        assertTrue(SwordBondHudStateStore.snapshot().active(), "should be active after replace");
        assertEquals(2, SwordBondHudStateStore.snapshot().grade());

        SwordBondHudStateStore.replace(null);
        assertFalse(SwordBondHudStateStore.snapshot().active(), "null replace should reset to inactive");
    }

    @Test
    void allCommandsUseSwordBondLayer() {
        SwordBondHudState state = new SwordBondHudState(true, 5, "通灵", 0.8f, 0.9f, true);
        List<HudRenderCommand> cmds = SwordPathHudPlanner.buildCommands(
            state, System.currentTimeMillis(), 800, 600
        );
        for (HudRenderCommand cmd : cmds) {
            assertEquals(
                HudRenderLayer.SWORD_BOND, cmd.layer(),
                "all sword HUD commands should use SWORD_BOND layer"
            );
        }
    }
}
