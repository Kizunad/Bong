package com.bong.client.hud;

import com.bong.client.state.ZoneState;

import java.util.ArrayList;
import java.util.List;

public final class BongZoneHud {
    static final int TITLE_COLOR = 0xFFD700;
    static final int OVERLAY_COLOR = 0x9FD3FF;
    static final long TITLE_SHOW_MILLIS = 2_000L;
    static final long TITLE_FULL_ALPHA_MILLIS = 1_500L;
    static final int QI_BAR_SEGMENTS = 10;
    static final int MAX_DANGER_SYMBOLS = 5;

    private BongZoneHud() {
    }

    public static List<HudRenderCommand> buildCommands(
        ZoneState zoneState,
        long nowMillis,
        HudTextHelper.WidthMeasurer widthMeasurer,
        int maxWidth,
        int x,
        int y,
        int screenWidth,
        int screenHeight
    ) {
        ZoneState safeZoneState = zoneState == null ? ZoneState.empty() : zoneState;
        if (safeZoneState.isEmpty() || widthMeasurer == null || maxWidth <= 0) {
            return List.of();
        }

        List<HudRenderCommand> commands = new ArrayList<>();
        int titleAlpha = centeredTitleAlpha(safeZoneState.changedAtMillis(), nowMillis);
        if (titleAlpha > 0 && screenWidth > 0 && screenHeight > 0) {
            String clippedTitle = HudTextHelper.clipToWidth(
                centeredTitleText(safeZoneState),
                Math.max(1, screenWidth - 16),
                widthMeasurer
            );
            if (!clippedTitle.isEmpty()) {
                int titleX = Math.max(0, (screenWidth - Math.max(0, widthMeasurer.measure(clippedTitle))) / 2);
                int titleY = Math.max(0, screenHeight / 3);
                commands.add(HudRenderCommand.text(
                    HudRenderLayer.ZONE,
                    clippedTitle,
                    titleX,
                    titleY,
                    HudTextHelper.withAlpha(TITLE_COLOR, titleAlpha)
                ));
            }
        }

        String clippedOverlay = HudTextHelper.clipToWidth(persistentOverlayText(safeZoneState), maxWidth, widthMeasurer);
        if (!clippedOverlay.isEmpty()) {
            commands.add(HudRenderCommand.text(HudRenderLayer.ZONE, clippedOverlay, x, y, OVERLAY_COLOR));
        }
        return List.copyOf(commands);
    }

    static int centeredTitleAlpha(long changedAtMillis, long nowMillis) {
        long safeNowMillis = Math.max(0L, nowMillis);
        long safeChangedAtMillis = Math.max(0L, changedAtMillis);
        long elapsedMillis = Math.max(0L, safeNowMillis - safeChangedAtMillis);
        if (elapsedMillis >= TITLE_SHOW_MILLIS) {
            return 0;
        }
        if (elapsedMillis <= TITLE_FULL_ALPHA_MILLIS) {
            return 255;
        }

        long fadeMillis = TITLE_SHOW_MILLIS - TITLE_FULL_ALPHA_MILLIS;
        long remainingMillis = TITLE_SHOW_MILLIS - elapsedMillis;
        return HudTextHelper.clampAlpha((int) Math.round(255.0 * remainingMillis / fadeMillis));
    }

    static String centeredTitleText(ZoneState zoneState) {
        ZoneState safeZoneState = zoneState == null ? ZoneState.empty() : zoneState;
        return "— " + safeZoneState.zoneLabel() + " —";
    }

    static String persistentOverlayText(ZoneState zoneState) {
        ZoneState safeZoneState = zoneState == null ? ZoneState.empty() : zoneState;
        return "区域"
            + safeZoneState.zoneLabel()
            + statusText(safeZoneState)
            + " 灵气"
            + qiBar(safeZoneState.spiritQiNormalized())
            + " 危"
            + dangerText(safeZoneState.dangerLevel());
    }

    static String statusText(ZoneState zoneState) {
        ZoneState safeZoneState = zoneState == null ? ZoneState.empty() : zoneState;
        return safeZoneState.collapsed() ? " 死域" : "";
    }

    static String qiBar(double spiritQiNormalized) {
        int filledSegments = Math.max(0, Math.min(QI_BAR_SEGMENTS, (int) Math.round(clamp(spiritQiNormalized) * QI_BAR_SEGMENTS)));
        return "[" + "█".repeat(filledSegments) + "░".repeat(QI_BAR_SEGMENTS - filledSegments) + "]";
    }

    static String dangerText(int dangerLevel) {
        return dangerLevel <= 0 ? "无" : dangerSymbols(dangerLevel);
    }

    static String dangerSymbols(int dangerLevel) {
        int symbolCount = Math.max(0, Math.min(MAX_DANGER_SYMBOLS, dangerLevel));
        return "☠".repeat(symbolCount);
    }

    private static double clamp(double value) {
        if (!Double.isFinite(value)) {
            return 0.0;
        }
        return Math.max(0.0, Math.min(1.0, value));
    }
}
