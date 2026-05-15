package com.bong.client.hud;

import com.bong.client.combat.store.VortexStateStore;

import java.util.List;

/** Screen-space tint for active woliu-v2 turbulence around the player. */
public final class TurbulenceFieldVisualizeHud {
    private static final int MAX_ALPHA = 0x3C;

    private TurbulenceFieldVisualizeHud() {
    }

    public static List<HudRenderCommand> buildCommands(
        VortexStateStore.State state,
        int screenWidth,
        int screenHeight,
        long nowMillis
    ) {
        if (!WoliuV2StatusPanelHud.hasVisibleTurbulence(state, nowMillis)) return List.of();

        int alpha = Math.max(0x10, Math.min(MAX_ALPHA, Math.round(MAX_ALPHA * state.turbulenceIntensity())));
        int tint = (alpha << 24) | 0x182040;
        return List.of(HudRenderCommand.screenTint(HudRenderLayer.VORTEX_TURBULENCE, tint));
    }
}
