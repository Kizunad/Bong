package com.bong.client.combat;

import com.bong.client.hud.HudImmersionMode;
import com.bong.client.state.VisualEffectState;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;

class CombatHudBootstrapTest {
    @AfterEach
    void reset() {
        HudImmersionMode.resetForTests();
    }

    @Test
    void resetOnDisconnectClearsHudImmersionCombatWindow() {
        CombatHudState combat = CombatHudState.create(0.8f, 0.7f, 0.4f, DerivedAttrFlags.none());
        assertEquals(
            HudImmersionMode.Mode.COMBAT,
            HudImmersionMode.resolve(combat, VisualEffectState.none(), 1_000L)
        );

        CombatHudBootstrap.resetOnDisconnect();

        assertEquals(
            HudImmersionMode.Mode.PEACE,
            HudImmersionMode.resolve(CombatHudState.empty(), VisualEffectState.none(), 1_500L)
        );
    }
}
