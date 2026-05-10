package com.bong.client.combat.baomai.v3;

import com.bong.client.BongHud.HudSurface;

public final class MeridianRippleScarHud {
    private static final int WIDTH = 96;
    private static final int HEIGHT = 6;
    private static final int BG = 0xAA17110A;
    private static final int FILL = 0xFFD8A03A;
    private static final int TEXT = 0xFFEAC982;

    private MeridianRippleScarHud() {}

    public static int render(HudSurface surface, BaomaiV3HudStateStore.Snapshot snapshot, int x, int y) {
        if (snapshot == null || !snapshot.meridianRippleScarVisible()) {
            return y;
        }
        int filled = Math.max(1, Math.round(WIDTH * (float) snapshot.meridianRippleScarSeverity()));
        surface.fill(x, y + 11, x + WIDTH, y + 11 + HEIGHT, BG);
        surface.fill(x, y + 11, x + filled, y + 11 + HEIGHT, FILL);
        surface.drawText("经脉龟裂 " + formatPercent(snapshot.meridianRippleScarSeverity()), x, y, TEXT, true);
        return y + 22;
    }

    private static String formatPercent(double value) {
        return Math.round(Math.max(0.0, Math.min(1.0, value)) * 100.0) + "%";
    }
}
