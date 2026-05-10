package com.bong.client.combat.baomai.v3;

import com.bong.client.BongHud.HudSurface;

public final class BodyTranscendenceTimerHud {
    private static final int WIDTH = 136;
    private static final int HEIGHT = 7;
    private static final int BG = 0xAA271B05;
    private static final int FILL = 0xFFF5D36A;
    private static final int TEXT = 0xFFFFF0B8;

    private BodyTranscendenceTimerHud() {}

    public static int render(HudSurface surface, BaomaiV3HudStateStore.Snapshot snapshot, int x, int y) {
        if (snapshot == null || !snapshot.bodyTranscendenceActive()) {
            return y;
        }
        int filled = Math.max(1, Math.round(WIDTH * (float) snapshot.bodyTranscendenceProgress()));
        surface.fill(x, y + 11, x + WIDTH, y + 11 + HEIGHT, BG);
        surface.fill(x, y + 11, x + filled, y + 11 + HEIGHT, FILL);
        surface.drawText(
            "凡躯重铸 x" + formatMultiplier(snapshot.flowRateMultiplier()) + " "
                + BloodBurnRatioHud.formatSeconds(snapshot.bodyTranscendenceRemainingTicks()),
            x,
            y,
            TEXT,
            true
        );
        return y + 24;
    }

    private static String formatMultiplier(double value) {
        if (!Double.isFinite(value) || value <= 0.0) {
            return "1";
        }
        return value == Math.rint(value) ? Long.toString(Math.round(value)) : String.format("%.1f", value);
    }
}
