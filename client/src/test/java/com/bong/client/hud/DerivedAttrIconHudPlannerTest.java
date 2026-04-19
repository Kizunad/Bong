package com.bong.client.hud;

import com.bong.client.combat.store.DerivedAttrsStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

class DerivedAttrIconHudPlannerTest {
    @AfterEach void tearDown() { DerivedAttrsStore.resetForTests(); }

    @Test void hiddenWhenNoneActive() {
        assertTrue(DerivedAttrIconHudPlanner.buildCommands(800, 600).isEmpty());
    }

    @Test void drawsFlightIconOnly() {
        DerivedAttrsStore.replace(new DerivedAttrsStore.State(
            true, 0.5f, 0, false, 0, false, "", 0f, 0, false
        ));
        List<HudRenderCommand> cmds = DerivedAttrIconHudPlanner.buildCommands(800, 600);
        long textCount = cmds.stream().filter(HudRenderCommand::isText).count();
        assertEquals(1, textCount);
    }

    @Test void drawsAllThreeIcons() {
        DerivedAttrsStore.replace(new DerivedAttrsStore.State(
            true, 0.5f, 0, true, 5000L, true, "striking", 0f, 0, false
        ));
        List<HudRenderCommand> cmds = DerivedAttrIconHudPlanner.buildCommands(800, 600);
        long textCount = cmds.stream().filter(HudRenderCommand::isText).count();
        assertEquals(3, textCount);
    }
}
