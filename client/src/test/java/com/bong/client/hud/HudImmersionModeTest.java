package com.bong.client.hud;

import com.bong.client.combat.CombatHudState;
import com.bong.client.combat.DerivedAttrFlags;
import com.bong.client.state.VisualEffectState;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;

class HudImmersionModeTest {
    @AfterEach
    void reset() {
        HudImmersionMode.resetForTests();
    }

    @Test
    void combatGraceWindowDoesNotSurviveBackwardClockJump() {
        CombatHudState combat = CombatHudState.create(0.8f, 0.7f, 0.4f, DerivedAttrFlags.none());

        assertEquals(
            HudImmersionMode.Mode.COMBAT,
            HudImmersionMode.resolve(combat, VisualEffectState.none(), 1_000L)
        );
        assertEquals(
            HudImmersionMode.Mode.PEACE,
            HudImmersionMode.resolve(CombatHudState.empty(), VisualEffectState.none(), 900L)
        );
    }

    @Test
    void combatFilterDropsNullCommandsBeforeCopying() {
        List<HudRenderCommand> commands = new ArrayList<>();
        commands.add(HudRenderCommand.rect(HudRenderLayer.QUICK_BAR, 1, 2, 3, 4, 0xFFFFFFFF));
        commands.add(null);

        List<HudRenderCommand> filtered = HudImmersionMode.filter(commands, HudImmersionMode.Mode.COMBAT);

        assertEquals(1, filtered.size());
        assertEquals(HudRenderLayer.QUICK_BAR, filtered.get(0).layer());
    }
}
