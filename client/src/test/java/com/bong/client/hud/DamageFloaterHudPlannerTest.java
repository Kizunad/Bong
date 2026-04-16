package com.bong.client.hud;

import com.bong.client.combat.store.DamageFloaterStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

class DamageFloaterHudPlannerTest {
    @AfterEach void tearDown() { DamageFloaterStore.resetForTests(); }

    @Test void emptyByDefault() {
        assertTrue(DamageFloaterHudPlanner.buildCommands(800, 600, 0L).isEmpty());
    }

    @Test void rendersFloatersAsText() {
        DamageFloaterStore.publish(new DamageFloaterStore.Floater(
            0, 0, 0, "12", 0xFFFF0000, DamageFloaterStore.Kind.HIT, 1_000L
        ));
        DamageFloaterStore.publish(new DamageFloaterStore.Floater(
            0, 0, 0, "25", 0xFFFFC040, DamageFloaterStore.Kind.CRIT, 1_000L
        ));
        List<HudRenderCommand> cmds = DamageFloaterHudPlanner.buildCommands(800, 600, 1_200L);
        assertEquals(2, cmds.size());
        for (HudRenderCommand c : cmds) {
            assertTrue(c.isText());
            assertEquals(HudRenderLayer.DAMAGE_FLOATER, c.layer());
        }
        boolean critMarked = cmds.stream().anyMatch(c -> c.text().endsWith("!"));
        assertTrue(critMarked);
    }

    @Test void healAddsPlusSign() {
        DamageFloaterStore.publish(new DamageFloaterStore.Floater(
            0, 0, 0, "8", 0xFF60D060, DamageFloaterStore.Kind.HEAL, 1_000L
        ));
        List<HudRenderCommand> cmds = DamageFloaterHudPlanner.buildCommands(800, 600, 1_100L);
        assertEquals(1, cmds.size());
        assertTrue(cmds.get(0).text().startsWith("+"));
    }
}
