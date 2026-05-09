package com.bong.client.hud;

import com.bong.client.combat.store.FullPowerStateStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

class ChargingProgressBarHudTest {
    @AfterEach void tearDown() {
        FullPowerStateStore.resetForTests();
    }

    @Test void hiddenWhenInactive() {
        assertTrue(ChargingProgressBarHud.buildCommands(800, 600).isEmpty());
    }

    @Test void drawsTrackFillAndTextWhenCharging() {
        FullPowerStateStore.updateCharging(new FullPowerStateStore.ChargingState(
            true, "offline:Azure", 40.0, 80.0, 1_200L, 1_000L
        ));

        List<HudRenderCommand> commands = ChargingProgressBarHud.buildCommands(800, 600);

        assertEquals(3, commands.size());
        assertTrue(commands.get(0).isRect());
        assertEquals(ChargingProgressBarHud.BAR_WIDTH, commands.get(0).width());
        assertTrue(commands.get(1).isRect());
        assertEquals(75, commands.get(1).width());
        assertTrue(commands.get(2).isText());
        assertTrue(commands.get(2).text().contains("蓄力中"));
        for (HudRenderCommand command : commands) {
            assertEquals(HudRenderLayer.CAST_BAR, command.layer());
        }
    }
}
