package com.bong.client.combat.baomai.v3;

import com.bong.client.BongHud.HudSurface;

public final class BaomaiV3Hud {
    private static final int RIGHT_MARGIN = 14;
    private static final int TOP = 46;
    private static final int WIDTH = 146;

    private BaomaiV3Hud() {}

    public static void render(HudSurface surface, long nowMs) {
        BaomaiV3HudStateStore.Snapshot snapshot = BaomaiV3HudStateStore.snapshot(nowMs);
        int x = Math.max(8, surface.windowWidth() - WIDTH - RIGHT_MARGIN);
        int y = TOP;
        y = BloodBurnRatioHud.render(surface, snapshot, x, y);
        y = BodyTranscendenceTimerHud.render(surface, snapshot, x, y);
        MeridianRippleScarHud.render(surface, snapshot, x, y);
    }
}
