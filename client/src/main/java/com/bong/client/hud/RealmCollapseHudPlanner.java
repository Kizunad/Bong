package com.bong.client.hud;

import com.bong.client.state.RealmCollapseHudState;

import java.util.ArrayList;
import java.util.List;

public final class RealmCollapseHudPlanner {
    private static final int PANEL_WIDTH = 260;
    private static final int PANEL_HEIGHT = 48;
    private static final int TRACK_HEIGHT = 5;
    private static final int BG = 0xD0181010;
    private static final int BORDER = 0xFFFF5050;
    private static final int TRACK_BG = 0xFF241414;
    private static final int FILL = 0xFFFF7070;
    private static final int TEXT = 0xFFFFEEEE;
    private static final int MUTED = 0xFFFFB8B8;

    private RealmCollapseHudPlanner() {
    }

    public static List<HudRenderCommand> buildCommands(
        RealmCollapseHudState state,
        HudTextHelper.WidthMeasurer widthMeasurer,
        int screenWidth,
        int screenHeight,
        long nowMillis
    ) {
        List<HudRenderCommand> out = new ArrayList<>();
        RealmCollapseHudState safe = state == null ? RealmCollapseHudState.empty() : state;
        if (screenWidth <= 0 || screenHeight <= 0 || widthMeasurer == null || !safe.active(nowMillis)) {
            return out;
        }

        int remainingTicks = safe.remainingTicks(nowMillis);
        int x = Math.max(8, (screenWidth - PANEL_WIDTH) / 2);
        int y = Math.max(34, screenHeight / 7);
        out.add(HudRenderCommand.screenTint(HudRenderLayer.REALM_COLLAPSE, 0x1AFF0000));
        out.add(HudRenderCommand.rect(HudRenderLayer.REALM_COLLAPSE, x + 2, y + 2, PANEL_WIDTH, PANEL_HEIGHT, 0x88000000));
        out.add(HudRenderCommand.rect(HudRenderLayer.REALM_COLLAPSE, x, y, PANEL_WIDTH, PANEL_HEIGHT, BG));
        out.add(HudRenderCommand.rect(HudRenderLayer.REALM_COLLAPSE, x, y, PANEL_WIDTH, 1, BORDER));
        out.add(HudRenderCommand.rect(HudRenderLayer.REALM_COLLAPSE, x, y + PANEL_HEIGHT - 1, PANEL_WIDTH, 1, BORDER));
        out.add(HudRenderCommand.rect(HudRenderLayer.REALM_COLLAPSE, x, y, 1, PANEL_HEIGHT, BORDER));
        out.add(HudRenderCommand.rect(HudRenderLayer.REALM_COLLAPSE, x + PANEL_WIDTH - 1, y, 1, PANEL_HEIGHT, BORDER));

        String title = "域崩撤离 " + secondsLabel(remainingTicks);
        String detail = "区域 " + zoneLabel(safe.zone()) + " · 立即离开边界";
        out.add(HudRenderCommand.text(HudRenderLayer.REALM_COLLAPSE, title, x + 8, y + 8, TEXT));
        out.add(HudRenderCommand.text(
            HudRenderLayer.REALM_COLLAPSE,
            HudTextHelper.clipToWidth(detail, PANEL_WIDTH - 16, widthMeasurer),
            x + 8,
            y + 22,
            MUTED
        ));

        int trackX = x + 8;
        int trackY = y + 38;
        int trackW = PANEL_WIDTH - 16;
        double ratio = safe.durationTicks() <= 0 ? 0.0 : remainingTicks / (double) safe.durationTicks();
        ratio = Math.max(0.0, Math.min(1.0, ratio));
        out.add(HudRenderCommand.rect(HudRenderLayer.REALM_COLLAPSE, trackX, trackY, trackW, TRACK_HEIGHT, TRACK_BG));
        out.add(HudRenderCommand.rect(
            HudRenderLayer.REALM_COLLAPSE,
            trackX,
            trackY,
            (int) Math.round(trackW * ratio),
            TRACK_HEIGHT,
            FILL
        ));
        return List.copyOf(out);
    }

    private static String zoneLabel(String zone) {
        String normalized = zone == null ? "" : zone.trim();
        return normalized.isEmpty() ? "未知" : normalized.replace('_', ' ');
    }

    private static String secondsLabel(int ticks) {
        double seconds = Math.max(0, ticks) / 20.0;
        if (seconds >= 10.0 || Math.abs(seconds - Math.rint(seconds)) < 0.05) {
            return Math.round(seconds) + "s";
        }
        return String.format(java.util.Locale.ROOT, "%.1fs", seconds);
    }
}
