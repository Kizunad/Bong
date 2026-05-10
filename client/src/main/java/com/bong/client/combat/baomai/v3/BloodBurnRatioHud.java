package com.bong.client.combat.baomai.v3;

import com.bong.client.BongHud.HudSurface;

public final class BloodBurnRatioHud {
    private static final int WIDTH = 116;
    private static final int HEIGHT = 7;
    private static final int BG = 0xAA16040A;
    private static final int FILL = 0xFFD12A3D;
    private static final int TEXT = 0xFFFFD7D7;

    private BloodBurnRatioHud() {}

    public static int render(HudSurface surface, BaomaiV3HudStateStore.Snapshot snapshot, int x, int y) {
        if (snapshot == null || !snapshot.bloodBurnActive()) {
            return y;
        }
        int filled = Math.max(1, Math.round(WIDTH * (float) snapshot.bloodBurnProgress()));
        surface.fill(x, y + 11, x + WIDTH, y + 11 + HEIGHT, BG);
        surface.fill(x, y + 11, x + filled, y + 11 + HEIGHT, FILL);
        surface.drawText("焚血 " + formatSeconds(snapshot.bloodBurnRemainingTicks()), x, y, TEXT, true);
        return y + 24;
    }

    static String formatSeconds(long ticks) {
        return String.format("%.1fs", ticks / 20.0);
    }
}
