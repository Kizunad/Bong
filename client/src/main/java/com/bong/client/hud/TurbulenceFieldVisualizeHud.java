package com.bong.client.hud;

import com.bong.client.combat.store.VortexStateStore;

import java.util.ArrayList;
import java.util.List;

/** Screen-space hint for active woliu-v2 turbulence around the player. */
public final class TurbulenceFieldVisualizeHud {
    private static final int MAX_ALPHA = 0x55;

    private TurbulenceFieldVisualizeHud() {
    }

    public static List<HudRenderCommand> buildCommands(
        VortexStateStore.State state,
        int screenWidth,
        int screenHeight,
        long nowMillis
    ) {
        if (state == null || state.turbulenceIntensity() <= 0f) return List.of();
        if (state.turbulenceUntilMs() > 0 && state.turbulenceUntilMs() <= nowMillis) return List.of();

        int alpha = Math.max(0x18, Math.round(MAX_ALPHA * state.turbulenceIntensity()));
        int tint = (alpha << 24) | 0x182040;
        int x = Math.max(8, screenWidth - 118);
        int y = Math.max(22, screenHeight - 122);
        String text = "紊流 r" + Math.round(state.turbulenceRadius());

        List<HudRenderCommand> out = new ArrayList<>();
        out.add(HudRenderCommand.screenTint(HudRenderLayer.VORTEX_TURBULENCE, tint));
        out.add(HudRenderCommand.text(HudRenderLayer.VORTEX_TURBULENCE, text, x, y, 0xFFC8E8FF));
        return out;
    }
}
