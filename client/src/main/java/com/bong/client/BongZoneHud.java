package com.bong.client;

import java.util.Objects;

public final class BongZoneHud {
    static final long BIG_TITLE_HOLD_MS = 1_500L;
    static final long BIG_TITLE_FADE_OUT_MS = 500L;
    static final int BIG_TITLE_RGB = 0xF4D35E;
    static final int PANEL_BACKGROUND_COLOR = 0x66000000;
    static final int PANEL_BORDER_COLOR = 0x55F4D35E;
    static final int BAR_BACKGROUND_COLOR = 0x55000000;
    static final int BAR_FILL_COLOR = 0xFF4DD2FF;
    static final int SAFE_TEXT_COLOR = 0xFFF4F4F4;
    static final int QI_LABEL_COLOR = 0xFF7FDBFF;
    static final int DANGER_LOW_RGB = 0xFFD166;
    static final int DANGER_MEDIUM_RGB = 0xFFFFB347;
    static final int DANGER_HIGH_RGB = 0xFFFF5555;
    static final int HUD_LEFT = 10;
    static final int STATUS_TOP = 28;
    static final int ZONE_LABEL_Y = 22;
    static final int DANGER_Y = 42;
    static final int QI_BAR_WIDTH = 96;
    static final int QI_BAR_HEIGHT = 8;

    private BongZoneHud() {
    }

    static void render(BongHud.HudSurface surface, ZoneState.ZoneHudState zoneState, long nowMs) {
        Objects.requireNonNull(surface, "surface");
        if (zoneState == null) {
            return;
        }

        renderBigTitle(surface, zoneState, nowMs);
        renderStatusPanel(surface, zoneState);
    }

    static int bigTitleAlpha(long nowMs, long changedAtMs) {
        if (nowMs <= changedAtMs) {
            return 255;
        }

        long elapsed = nowMs - changedAtMs;
        if (elapsed <= BIG_TITLE_HOLD_MS) {
            return 255;
        }

        long fadeElapsed = elapsed - BIG_TITLE_HOLD_MS;
        if (fadeElapsed >= BIG_TITLE_FADE_OUT_MS) {
            return 0;
        }

        long remaining = BIG_TITLE_FADE_OUT_MS - fadeElapsed;
        return (int) Math.round(255.0d * remaining / (double) BIG_TITLE_FADE_OUT_MS);
    }

    static int spiritQiFillWidth(double spiritQi) {
        return (int) Math.round(ZoneState.clampSpiritQi(spiritQi) * QI_BAR_WIDTH);
    }

    static String spiritQiPercentLabel(double spiritQi) {
        return Math.round(ZoneState.clampSpiritQi(spiritQi) * 100.0d) + "%";
    }

    static String dangerMarkers(int dangerLevel) {
        int clampedDanger = ZoneState.clampDangerLevel(dangerLevel);
        return clampedDanger == 0 ? "-" : "!".repeat(clampedDanger);
    }

    static int dangerColor(int dangerLevel) {
        int clampedDanger = ZoneState.clampDangerLevel(dangerLevel);
        if (clampedDanger >= 4) {
            return DANGER_HIGH_RGB;
        }
        if (clampedDanger >= 2) {
            return DANGER_MEDIUM_RGB;
        }
        return clampedDanger == 0 ? SAFE_TEXT_COLOR : DANGER_LOW_RGB;
    }

    static int withAlpha(int rgb, int alpha) {
        return ((alpha & 0xFF) << 24) | (rgb & 0x00FFFFFF);
    }

    private static void renderBigTitle(BongHud.HudSurface surface, ZoneState.ZoneHudState zoneState, long nowMs) {
        int alpha = bigTitleAlpha(nowMs, zoneState.changedAtMs());
        if (alpha <= 0) {
            return;
        }

        String title = "-- " + zoneState.zoneLabel() + " --";
        int titleWidth = surface.measureText(title);
        int x = (surface.windowWidth() - titleWidth) / 2;
        int y = surface.windowHeight() / 3;

        surface.fill(x - 6, y - 4, x + titleWidth + 6, y + 12, withAlpha(0x101010, Math.min(alpha, 160)));
        surface.drawText(title, x, y, withAlpha(BIG_TITLE_RGB, alpha), true);
    }

    private static void renderStatusPanel(BongHud.HudSurface surface, ZoneState.ZoneHudState zoneState) {
        surface.fill(HUD_LEFT - 4, ZONE_LABEL_Y - 4, HUD_LEFT + 160, DANGER_Y + 10, PANEL_BACKGROUND_COLOR);
        surface.fill(HUD_LEFT - 4, ZONE_LABEL_Y - 4, HUD_LEFT + 160, ZONE_LABEL_Y - 3, PANEL_BORDER_COLOR);

        surface.drawTextWithShadow(zoneState.zoneLabel(), HUD_LEFT, ZONE_LABEL_Y, SAFE_TEXT_COLOR);
        surface.drawText("灵气", HUD_LEFT, STATUS_TOP, QI_LABEL_COLOR, true);

        int barLeft = HUD_LEFT + 24;
        int barTop = STATUS_TOP + 2;
        int fillWidth = spiritQiFillWidth(zoneState.spiritQi());
        surface.fill(barLeft, barTop, barLeft + QI_BAR_WIDTH, barTop + QI_BAR_HEIGHT, BAR_BACKGROUND_COLOR);
        surface.fill(barLeft, barTop, barLeft + fillWidth, barTop + QI_BAR_HEIGHT, BAR_FILL_COLOR);
        surface.drawText(spiritQiPercentLabel(zoneState.spiritQi()), barLeft + QI_BAR_WIDTH + 8, STATUS_TOP, SAFE_TEXT_COLOR, true);

        surface.drawText(
                "危险 " + dangerMarkers(zoneState.dangerLevel()),
                HUD_LEFT,
                DANGER_Y,
                dangerColor(zoneState.dangerLevel()),
                true
        );
    }
}
