package com.bong.client.hud;

import com.bong.client.combat.store.FullPowerStateStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

class ExhaustedGreyOverlayTest {
    @AfterEach void tearDown() {
        FullPowerStateStore.resetForTests();
    }

    @Test void hiddenWhenInactive() {
        assertTrue(ExhaustedGreyOverlay.buildCommands(800, 600, 1_000L).isEmpty());
    }

    @Test void hiddenWhenRecoveryExpired() {
        FullPowerStateStore.updateExhausted(new FullPowerStateStore.ExhaustedState(
            true, "offline:Azure", 100L, 300L, 1_000L
        ));

        assertTrue(ExhaustedGreyOverlay.buildCommands(800, 600, 11_000L).isEmpty());
    }

    @Test void drawsVignetteBarAndTextWhileExhausted() {
        FullPowerStateStore.updateExhausted(new FullPowerStateStore.ExhaustedState(
            true, "offline:Azure", 100L, 300L, 1_000L
        ));

        List<HudRenderCommand> commands = ExhaustedGreyOverlay.buildCommands(800, 600, 1_000L);

        assertEquals(4, commands.size());
        assertTrue(commands.get(0).isEdgeVignette());
        assertEquals(HudRenderLayer.VISUAL, commands.get(0).layer());
        assertTrue(commands.get(1).isRect());
        assertEquals(ExhaustedGreyOverlay.BAR_WIDTH, commands.get(1).width());
        assertTrue(commands.get(2).isRect());
        assertEquals(ExhaustedGreyOverlay.BAR_WIDTH, commands.get(2).width());
        assertTrue(commands.get(3).isText());
        assertEquals("虚脱 10s", commands.get(3).text());
    }
}
