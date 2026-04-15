package com.bong.client.hud;

import com.bong.client.combat.store.DerivedAttrsStore;

import java.util.ArrayList;
import java.util.List;

/**
 * Renders flight qi countdown + forced-descent warning (plan §U7).
 */
public final class FlightHudPlanner {
    public static final int BAR_WIDTH = 80;
    public static final int BAR_HEIGHT = 3;
    public static final int TOP_MARGIN = 40;
    public static final int TRACK_COLOR = 0xC0102040;
    public static final int FILL_COLOR = 0xFF60C0FF;
    public static final int WARN_COLOR = 0xFFE04040;

    private FlightHudPlanner() {}

    public static List<HudRenderCommand> buildCommands(
        int screenWidth,
        int screenHeight,
        long nowMs
    ) {
        DerivedAttrsStore.State state = DerivedAttrsStore.snapshot();
        List<HudRenderCommand> out = new ArrayList<>();
        if (!state.flying() || screenWidth <= 0 || screenHeight <= 0) return out;

        int x = (screenWidth - BAR_WIDTH) / 2;
        int y = TOP_MARGIN;

        out.add(HudRenderCommand.rect(HudRenderLayer.FLIGHT_HUD, x, y, BAR_WIDTH, BAR_HEIGHT, TRACK_COLOR));
        float norm = state.flyingQiRemaining();
        int fill = Math.max(0, Math.round(norm * BAR_WIDTH));
        int color = norm < 0.25f ? WARN_COLOR : FILL_COLOR;
        if (fill > 0) {
            out.add(HudRenderCommand.rect(HudRenderLayer.FLIGHT_HUD, x, y, fill, BAR_HEIGHT, color));
        }

        // Forced-descent warning text if within 3s
        if (state.flyingForceDescentAtMs() > 0) {
            long delta = state.flyingForceDescentAtMs() - nowMs;
            if (delta > 0 && delta < 3_000L) {
                String msg = "\u2193 \u5373\u5c06\u5f3a\u5236\u4e0b\u843d (" + (delta / 100) / 10.0 + "s)";
                out.add(HudRenderCommand.text(
                    HudRenderLayer.FLIGHT_HUD, msg, x - 40, y + 6, WARN_COLOR
                ));
            }
        }
        return out;
    }
}
