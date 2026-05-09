package com.bong.client.hud;

import com.bong.client.combat.store.FullPowerStateStore;

import java.util.ArrayList;
import java.util.List;

public final class ExhaustedGreyOverlay {
    public static final int VIGNETTE_COLOR = 0x886E6A76;
    public static final int BAR_WIDTH = 120;
    public static final int BAR_HEIGHT = 3;
    public static final int TRACK_COLOR = 0x80303036;
    public static final int FILL_COLOR = 0xFFD0CCD8;

    private ExhaustedGreyOverlay() {}

    public static List<HudRenderCommand> buildCommands(int screenWidth, int screenHeight, long nowMs) {
        FullPowerStateStore.ExhaustedState state = FullPowerStateStore.exhausted();
        List<HudRenderCommand> out = new ArrayList<>();
        if (state == null || !state.active() || screenWidth <= 0 || screenHeight <= 0) {
            return out;
        }
        long totalTicks = Math.max(1L, state.recoveryAtTick() - state.startedTick());
        long remainingTicks = state.remainingTicks(nowMs);
        if (remainingTicks <= 0L) {
            return out;
        }
        double ratio = Math.max(0.0, Math.min(1.0, remainingTicks / (double) totalTicks));
        int x = (screenWidth - BAR_WIDTH) / 2;
        int y = screenHeight - 88;
        out.add(HudRenderCommand.edgeVignette(HudRenderLayer.VISUAL, VIGNETTE_COLOR));
        out.add(HudRenderCommand.rect(HudRenderLayer.CAST_BAR, x, y, BAR_WIDTH, BAR_HEIGHT, TRACK_COLOR));
        out.add(HudRenderCommand.rect(
            HudRenderLayer.CAST_BAR,
            x,
            y,
            Math.max(1, (int) Math.round(BAR_WIDTH * ratio)),
            BAR_HEIGHT,
            FILL_COLOR
        ));
        out.add(HudRenderCommand.text(
            HudRenderLayer.CAST_BAR,
            "虚脱 " + Math.max(1L, remainingTicks / 20L) + "s",
            x,
            y - 10,
            FILL_COLOR
        ));
        return out;
    }
}
