package com.bong.client.hud;

import com.bong.client.combat.CombatHudState;
import com.bong.client.combat.DerivedAttrFlags;
import com.bong.client.combat.store.DerivedAttrsStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

class StaminaBarHudPlannerTest {
    @AfterEach void tearDown() { DerivedAttrsStore.resetForTests(); }

    @Test void hiddenWhenInactive() {
        List<HudRenderCommand> cmds = StaminaBarHudPlanner.buildCommands(
            CombatHudState.empty(), 800, 600
        );
        assertTrue(cmds.isEmpty());
    }

    @Test void hiddenWhenStaminaFull() {
        CombatHudState state = CombatHudState.create(1f, 1f, 1f, DerivedAttrFlags.none());
        List<HudRenderCommand> cmds = StaminaBarHudPlanner.buildCommands(state, 800, 600);
        assertTrue(cmds.isEmpty());
    }

    @Test void drawsTrackAndFillWhenPartial() {
        CombatHudState state = CombatHudState.create(1f, 1f, 0.5f, DerivedAttrFlags.none());
        List<HudRenderCommand> cmds = StaminaBarHudPlanner.buildCommands(state, 800, 600);
        assertEquals(2, cmds.size(), () -> "Expected track + fill");
        for (HudRenderCommand c : cmds) {
            assertEquals(HudRenderLayer.STAMINA_BAR, c.layer());
        }
    }

    @Test void lowColorWhenUnder25Percent() {
        CombatHudState state = CombatHudState.create(1f, 1f, 0.1f, DerivedAttrFlags.none());
        List<HudRenderCommand> cmds = StaminaBarHudPlanner.buildCommands(state, 800, 600);
        HudRenderCommand fill = cmds.get(1);
        assertEquals(StaminaBarHudPlanner.LOW_COLOR, fill.color());
    }
}
