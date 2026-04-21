package com.bong.client.hud;

import com.bong.client.combat.store.DerivedAttrsStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

class FlightHudPlannerTest {
    @AfterEach void tearDown() { DerivedAttrsStore.resetForTests(); }

    @Test void hiddenWhenNotFlying() {
        assertTrue(FlightHudPlanner.buildCommands(800, 600, 1_000L).isEmpty());
    }

    @Test void drawsTrackAndFillWhenFlying() {
        DerivedAttrsStore.replace(new DerivedAttrsStore.State(
            true, 0.8f, 0L, false, 0L, false, "", 0f, 0, false
        ));
        List<HudRenderCommand> cmds = FlightHudPlanner.buildCommands(800, 600, 1_000L);
        assertEquals(2, cmds.stream().filter(HudRenderCommand::isRect).count());
    }

    @Test void warnTextShownWhenImminentDescent() {
        DerivedAttrsStore.replace(new DerivedAttrsStore.State(
            true, 0.1f, 2_500L, false, 0L, false, "", 0f, 0, false
        ));
        List<HudRenderCommand> cmds = FlightHudPlanner.buildCommands(800, 600, 1_000L);
        assertTrue(cmds.stream().anyMatch(HudRenderCommand::isText));
    }
}
