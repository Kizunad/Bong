package com.bong.client;

import java.util.ArrayList;
import java.util.List;
import java.util.Objects;

public final class BongEventAlertOverlay {
    private static final int MARGIN = 12;
    private static final int ACCENT_WIDTH = 3;
    private static final int BACKGROUND_RGB = 0x151515;
    private static final int DETAIL_RGB = 0xFFF4F4F4;
    private static final int MUTED_RGB = 0xFFD7D7D7;

    private BongEventAlertOverlay() {
    }

    static void render(BongHud.HudSurface surface, EventAlertState.BannerState bannerState) {
        Objects.requireNonNull(surface, "surface");
        if (bannerState == null || bannerState.alpha() <= 0) {
            return;
        }

        List<String> lines = linesFor(bannerState);
        int width = 0;
        for (String line : lines) {
            width = Math.max(width, surface.measureText(line));
        }

        int boxWidth = width + 18;
        int boxHeight = (lines.size() * 12) + 8;
        int x = surface.windowWidth() - boxWidth - MARGIN;
        int y = 10;

        surface.fill(x, y, x + boxWidth, y + boxHeight, BongZoneHud.withAlpha(BACKGROUND_RGB, Math.min(180, bannerState.alpha())));
        surface.fill(
                x,
                y,
                x + ACCENT_WIDTH,
                y + boxHeight,
                BongZoneHud.withAlpha(bannerState.severity().accentColor(), bannerState.alpha())
        );

        int textX = x + ACCENT_WIDTH + 6;
        int textY = y + 4;
        surface.drawText(lines.get(0), textX, textY, BongZoneHud.withAlpha(bannerState.severity().accentColor(), bannerState.alpha()), true);
        surface.drawText(lines.get(1), textX, textY + 12, BongZoneHud.withAlpha(DETAIL_RGB, bannerState.alpha()), false);
        if (lines.size() > 2) {
            surface.drawText(lines.get(2), textX, textY + 24, BongZoneHud.withAlpha(MUTED_RGB, bannerState.alpha()), false);
        }
    }

    static List<String> linesFor(EventAlertState.BannerState bannerState) {
        Objects.requireNonNull(bannerState, "bannerState");

        List<String> lines = new ArrayList<>();
        lines.add(bannerState.severity().label() + " | " + bannerState.title());
        lines.add(bannerState.detail());
        if (bannerState.zoneLabel() != null && !bannerState.zoneLabel().isBlank()) {
            lines.add("区域 " + bannerState.zoneLabel());
        }
        return lines;
    }
}
