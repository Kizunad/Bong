package com.bong.client.hud;

import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class RealmTaintedHudPlannerTest {
    @Test
    void hiddenWhenSeverityIsZero() {
        assertTrue(RealmTaintedHudPlanner.buildCommands(0.0f, 320).isEmpty());
    }

    @Test
    void labelsNicheIntrusionMainColorAtFullSeverity() {
        List<HudRenderCommand> commands = RealmTaintedHudPlanner.buildCommands(1.0f, 320);
        assertEquals(1, commands.size());
        assertEquals("龛侵主色", commands.get(0).text());
    }
}
