package com.bong.client.hud;

import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class CoffinHudPlannerTest {
    @Test
    void hidesWhenPlayerIsOutOfCoffin() {
        assertTrue(CoffinHudPlanner.buildCommands(CoffinStateStore.OUT, 800, 600).isEmpty());
    }

    @Test
    void showsLabelAndMultiplierWhenPlayerIsInCoffin() {
        List<HudRenderCommand> commands = CoffinHudPlanner.buildCommands(
            new CoffinStateStore.State(true, 0.9),
            800,
            600
        );

        assertEquals(3, commands.size());
        assertEquals(HudRenderLayer.COFFIN, commands.get(0).layer());
        assertTrue(commands.stream().anyMatch(cmd -> CoffinHudPlanner.LABEL.equals(cmd.text())));
        assertTrue(commands.stream().anyMatch(cmd -> "×0.9".equals(cmd.text())));
    }
}
