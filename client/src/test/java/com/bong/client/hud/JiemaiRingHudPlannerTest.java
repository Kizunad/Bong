package com.bong.client.hud;

import com.bong.client.combat.DefenseWindowState;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

class JiemaiRingHudPlannerTest {
    @Test void oneSecondPrepWindowRendersCollapsingRing() {
        DefenseWindowState state = DefenseWindowState.active(1000, 0L, 1000L);

        List<HudRenderCommand> start = JiemaiRingHudPlanner.buildCommands(state, 0L, 1920, 1080);
        List<HudRenderCommand> half = JiemaiRingHudPlanner.buildCommands(state, 500L, 1920, 1080);

        assertEquals(4, start.size());
        assertEquals(4, half.size());
        assertTrue(start.get(0).width() > half.get(0).width());
    }

    @Test void expiredPrepWindowDoesNotRender() {
        DefenseWindowState state = DefenseWindowState.active(1000, 0L, 1000L);

        assertTrue(JiemaiRingHudPlanner.buildCommands(state, 1000L, 1920, 1080).isEmpty());
    }
}
