package com.bong.client.hud;

import com.bong.client.combat.UnlockedStyles;
import com.bong.client.combat.store.DerivedAttrsStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertTrue;

class StyleBadgeHudPlannerTest {
    @AfterEach
    void tearDown() {
        DerivedAttrsStore.resetForTests();
    }

    @Test
    void hiddenWhenStyleLocked() {
        DerivedAttrsStore.replace(state(3, true));

        assertTrue(StyleBadgeHudPlanner.buildCommands(UnlockedStyles.none(), 800, 600).isEmpty());
    }

    @Test
    void drawsFakeSkinLayersOnlyWhenTishiUnlocked() {
        DerivedAttrsStore.replace(state(3, false));

        List<HudRenderCommand> commands = StyleBadgeHudPlanner.buildCommands(
            UnlockedStyles.of(false, true, false), 800, 600);

        assertTrue(commands.stream().anyMatch(cmd -> cmd.isText() && "伪×3".equals(cmd.text())));
    }

    @Test
    void drawsVortexWhenJuelingReady() {
        DerivedAttrsStore.replace(state(0, true));

        List<HudRenderCommand> commands = StyleBadgeHudPlanner.buildCommands(
            UnlockedStyles.of(false, false, true), 800, 600);

        assertTrue(commands.stream().anyMatch(cmd -> cmd.isText() && "涡".equals(cmd.text())));
    }

    private static DerivedAttrsStore.State state(int fakeSkinLayers, boolean vortexReady) {
        return new DerivedAttrsStore.State(
            false, 0f, 0L, false, 0L, false, "", 0f, fakeSkinLayers, vortexReady
        );
    }
}
