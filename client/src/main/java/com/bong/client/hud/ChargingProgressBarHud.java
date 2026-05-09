package com.bong.client.hud;

import com.bong.client.combat.store.FullPowerStateStore;

import java.util.ArrayList;
import java.util.List;
import java.util.Locale;

public final class ChargingProgressBarHud {
    public static final int BAR_WIDTH = 150;
    public static final int BAR_HEIGHT = 5;
    public static final int BOTTOM_MARGIN = 78;
    public static final int TRACK_COLOR = 0xB0181018;
    public static final int TEXT_COLOR = 0xFFE9D7FF;

    private ChargingProgressBarHud() {}

    public static List<HudRenderCommand> buildCommands(int screenWidth, int screenHeight) {
        FullPowerStateStore.ChargingState state = FullPowerStateStore.charging();
        List<HudRenderCommand> out = new ArrayList<>();
        if (state == null || !state.active() || screenWidth <= 0 || screenHeight <= 0) {
            return out;
        }

        int x = (screenWidth - BAR_WIDTH) / 2;
        int y = screenHeight - BOTTOM_MARGIN;
        double progress = state.progress();
        out.add(HudRenderCommand.rect(HudRenderLayer.CAST_BAR, x, y, BAR_WIDTH, BAR_HEIGHT, TRACK_COLOR));
        int fillWidth = Math.max(1, (int) Math.round(BAR_WIDTH * progress));
        out.add(HudRenderCommand.rect(HudRenderLayer.CAST_BAR, x, y, fillWidth, BAR_HEIGHT, fillColor(progress)));
        out.add(HudRenderCommand.text(
            HudRenderLayer.CAST_BAR,
            String.format(Locale.ROOT, "蓄力中 %.0f/%.0f 真元", state.qiCommitted(), state.targetQi()),
            x,
            y - 10,
            TEXT_COLOR
        ));
        return out;
    }

    static int fillColor(double progress) {
        if (progress >= 0.995) {
            return 0xFFFFD166;
        }
        if (progress >= 0.70) {
            return 0xFFFF4FD8;
        }
        if (progress >= 0.30) {
            return 0xFFFF305C;
        }
        return 0xFFFF7A7A;
    }
}
