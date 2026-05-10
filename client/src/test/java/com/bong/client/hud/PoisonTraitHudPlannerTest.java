package com.bong.client.hud;

import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class PoisonTraitHudPlannerTest {
    @Test
    void inactiveStateDoesNotRender() {
        assertTrue(PoisonTraitHudPlanner.buildCommands(PoisonTraitHudStateStore.State.NONE, 960, 540, 0L).isEmpty());
    }

    @Test
    void activeStateRendersToxicityAndDigestionBars() {
        PoisonTraitHudStateStore.State state = new PoisonTraitHudStateStore.State(
            true, 72.0f, 81.0f, 100.0f, 0L, 0.0f
        );

        List<HudRenderCommand> commands = PoisonTraitHudPlanner.buildCommands(state, 960, 540, 1_000L);

        assertTrue(commands.stream().anyMatch(cmd -> cmd.isText() && cmd.text().contains("重毒")));
        assertTrue(commands.stream().anyMatch(cmd -> cmd.isText() && cmd.text().contains("消化")));
        assertTrue(commands.stream().filter(HudRenderCommand::isRect).count() >= 4);
    }

    @Test
    void lifespanWarningRendersAsCentralToast() {
        PoisonTraitHudStateStore.State state = new PoisonTraitHudStateStore.State(
            true, 30.0f, 20.0f, 100.0f, 2_000L, 1.5f
        );

        List<HudRenderCommand> commands = PoisonTraitHudPlanner.buildCommands(state, 960, 540, 1_000L);

        assertTrue(commands.stream().anyMatch(cmd -> cmd.isToast() && cmd.text().contains("1.5")));
    }

    @Test
    void stateClampsUnsafeNumbers() {
        PoisonTraitHudStateStore.State state = new PoisonTraitHudStateStore.State(
            true, 200.0f, 120.0f, 0.0f, 0L, -1.0f
        );
        assertEquals(100.0f, state.toxicity(), 0.001f);
        assertEquals(1.0f, state.digestionCapacity(), 0.001f);
        assertEquals(1.0f, state.digestionCurrent(), 0.001f);
        assertEquals(0.0f, state.lifespanYearsLost(), 0.001f);
    }
}
