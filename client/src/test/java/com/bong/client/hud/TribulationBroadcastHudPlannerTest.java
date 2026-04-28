package com.bong.client.hud;

import com.bong.client.combat.store.TribulationBroadcastStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

class TribulationBroadcastHudPlannerTest {
    @AfterEach void tearDown() { TribulationBroadcastStore.resetForTests(); }

    @Test void hiddenWhenInactive() {
        assertTrue(TribulationBroadcastHudPlanner.buildCommands(800, 600, 1_000L).isEmpty());
    }

    @Test void drawsStageAndActor() {
        TribulationBroadcastStore.replace(new TribulationBroadcastStore.State(
            true, "甲", "warn", 0, 0, 10_000L, false, 0
        ));
        List<HudRenderCommand> cmds = TribulationBroadcastHudPlanner.buildCommands(800, 600, 1_000L);
        assertFalse(cmds.isEmpty());
        boolean hasWarn = cmds.stream().anyMatch(c -> c.isText() && c.text().contains("甲"));
        assertTrue(hasWarn);
    }

    @Test void drawsLockedStage() {
        TribulationBroadcastStore.replace(new TribulationBroadcastStore.State(
            true, "甲", "locked", 0, 0, 10_000L, false, 0
        ));
        List<HudRenderCommand> cmds = TribulationBroadcastHudPlanner.buildCommands(800, 600, 1_000L);
        boolean hasLocked = cmds.stream().anyMatch(c -> c.isText() && c.text().contains("劫锁已成"));
        assertTrue(hasLocked);
    }

    @Test void hiddenWhenExpired() {
        TribulationBroadcastStore.replace(new TribulationBroadcastStore.State(
            true, "甲", "warn", 0, 0, 1_000L, false, 0
        ));
        assertTrue(TribulationBroadcastHudPlanner.buildCommands(800, 600, 2_000L).isEmpty());
    }

    @Test void spectateHintShownWhenWithin50() {
        TribulationBroadcastStore.replace(new TribulationBroadcastStore.State(
            true, "甲", "warn", 0, 0, 10_000L, true, 30.0
        ));
        List<HudRenderCommand> cmds = TribulationBroadcastHudPlanner.buildCommands(800, 600, 1_000L);
        boolean hasSpectate = cmds.stream().anyMatch(c -> c.isText() && c.text().contains("观"));
        assertTrue(hasSpectate);
    }
}
