package com.bong.client.hud;

import com.bong.client.combat.store.FalseSkinHudStateStore;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class FalseSkinHudPlannerTest {
    @Test
    void stackHudDrawsOneLayerPerFalseSkin() {
        FalseSkinHudStateStore.State state = state(3, 25f);

        List<HudRenderCommand> commands = FalseSkinStackHud.buildCommands(state, 800, 600);

        long layerBoxes = commands.stream()
            .filter(HudRenderCommand::isRect)
            .filter(cmd -> cmd.width() == FalseSkinStackHud.LAYER_W && cmd.height() == FalseSkinStackHud.LAYER_H)
            .count();
        assertEquals(3, layerBoxes);
        assertTrue(commands.stream().anyMatch(cmd -> cmd.isText() && "伪皮".equals(cmd.text())));
    }

    @Test
    void contamHudShowsCurrentOuterLoadPercent() {
        FalseSkinHudStateStore.State state = state(2, 75f);

        List<HudRenderCommand> commands = ContamLoadHud.buildCommands(state, 800, 600);

        assertTrue(commands.stream().anyMatch(cmd -> cmd.isText() && "污 75%".equals(cmd.text())));
        assertTrue(commands.stream().anyMatch(cmd -> cmd.isRect() && cmd.width() == 38));
    }

    @Test
    void hiddenWithoutActiveFalseSkin() {
        assertTrue(FalseSkinStackHud.buildCommands(FalseSkinHudStateStore.State.NONE, 800, 600).isEmpty());
        assertTrue(ContamLoadHud.buildCommands(FalseSkinHudStateStore.State.NONE, 800, 600).isEmpty());
    }

    private static FalseSkinHudStateStore.State state(int layers, float contam) {
        return new FalseSkinHudStateStore.State(
            "offline:Azure",
            "rotten_wood_armor",
            layers,
            100f,
            contam,
            10L,
            List.of()
        );
    }
}
