package com.bong.client.hud;

import com.bong.client.combat.store.StatusEffectStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

class StatusEffectHudPlannerTest {
    @AfterEach void tearDown() { StatusEffectStore.resetForTests(); }

    @Test void emptyWhenNoEffects() {
        List<HudRenderCommand> cmds = StatusEffectHudPlanner.buildCommands(800, 600);
        assertTrue(cmds.isEmpty());
    }

    @Test void drawsSlotsForEachEffect() {
        StatusEffectStore.replace(List.of(
            new StatusEffectStore.Effect("a", "A", StatusEffectStore.Kind.DOT, 1, 5_000, 0xFFFF0000, "", 0),
            new StatusEffectStore.Effect("b", "B", StatusEffectStore.Kind.BUFF, 3, 8_000, 0xFF00FF00, "", 0)
        ));
        List<HudRenderCommand> cmds = StatusEffectHudPlanner.buildCommands(800, 600);
        assertFalse(cmds.isEmpty());
        for (HudRenderCommand c : cmds) {
            assertEquals(HudRenderLayer.STATUS_EFFECTS, c.layer());
        }
        long stackText = cmds.stream().filter(HudRenderCommand::isText).count();
        // Second effect has stacks=3 → one text entry for ×3
        assertEquals(1L, stackText);
    }

    @Test void debuffRemainingBarUsesRedCountdown() {
        StatusEffectStore.replace(List.of(
            new StatusEffectStore.Effect("stamina_crash", "体力虚脱", StatusEffectStore.Kind.DEBUFF, 1, 5_000, 0xFFFF8030, "", 0)
        ));

        List<HudRenderCommand> cmds = StatusEffectHudPlanner.buildCommands(800, 600);

        assertTrue(cmds.stream().anyMatch(cmd ->
            cmd.isRect()
                && cmd.width() > 0
                && cmd.height() == 1
                && cmd.color() == StatusEffectHudPlanner.DEBUFF_REMAINING_BAR_COLOR
        ), "debuff countdown bar should be red");
    }
}
