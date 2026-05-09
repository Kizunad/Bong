package com.bong.client.hud;

import com.bong.client.combat.store.VortexStateStore;

import java.util.ArrayList;
import java.util.List;
import java.util.Locale;

/** Woliu-v2 backfire warning overlay. */
public final class BackfireWarningHud {
    static final int MICRO_COLOR = 0x66FFAA40;
    static final int TORN_COLOR = 0x88FF5038;
    static final int SEVERED_COLOR = 0xB0A01020;

    private BackfireWarningHud() {
    }

    public static List<HudRenderCommand> buildCommands(
        VortexStateStore.State state,
        int screenWidth,
        int screenHeight
    ) {
        if (state == null || state.backfireLevel().isBlank()) return List.of();
        String level = state.backfireLevel().trim();
        int color = colorFor(level);
        int x = Math.max(10, screenWidth / 2 - 58);
        int y = Math.max(24, screenHeight / 2 + 34);

        List<HudRenderCommand> out = new ArrayList<>();
        out.add(HudRenderCommand.edgeVignette(HudRenderLayer.VORTEX_BACKFIRE, color));
        out.add(HudRenderCommand.text(HudRenderLayer.VORTEX_BACKFIRE, "反噬 " + level, x, y, 0xFFFFE0D0));
        return out;
    }

    private static int colorFor(String level) {
        String normalized = level.toLowerCase(Locale.ROOT);
        if (normalized.contains("severed")) return SEVERED_COLOR;
        if (normalized.contains("torn")) return TORN_COLOR;
        return MICRO_COLOR;
    }
}
