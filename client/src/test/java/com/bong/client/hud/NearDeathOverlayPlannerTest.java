package com.bong.client.hud;

import com.bong.client.combat.CombatHudState;
import com.bong.client.combat.DerivedAttrFlags;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

class NearDeathOverlayPlannerTest {

    @Test void hiddenWhenHpAboveThreshold() {
        CombatHudState state = CombatHudState.create(0.5f, 1f, 1f, DerivedAttrFlags.none());
        assertTrue(NearDeathOverlayPlanner.buildCommands(state, 800, 600).isEmpty());
    }

    @Test void hiddenWhenDead() {
        CombatHudState state = CombatHudState.create(0f, 1f, 1f, DerivedAttrFlags.none());
        assertTrue(NearDeathOverlayPlanner.buildCommands(state, 800, 600).isEmpty());
    }

    @Test void drawsTintVignetteAndText() {
        CombatHudState state = CombatHudState.create(0.05f, 1f, 1f, DerivedAttrFlags.none());
        List<HudRenderCommand> cmds = NearDeathOverlayPlanner.buildCommands(state, 800, 600);
        assertEquals(3, cmds.size());
        boolean hasTint = cmds.stream().anyMatch(HudRenderCommand::isScreenTint);
        boolean hasVignette = cmds.stream().anyMatch(HudRenderCommand::isEdgeVignette);
        boolean hasText = cmds.stream().anyMatch(HudRenderCommand::isText);
        assertTrue(hasTint);
        assertTrue(hasVignette);
        assertTrue(hasText);
    }
}
