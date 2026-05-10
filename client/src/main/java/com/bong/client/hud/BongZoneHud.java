package com.bong.client.hud;

import com.bong.client.state.ZoneState;

import java.util.ArrayList;
import java.util.List;

public final class BongZoneHud {
    static final int TITLE_COLOR = 0xFFD700;
    static final int NEGATIVE_TITLE_COLOR = 0xFFEE6677;
    static final int OVERLAY_COLOR = 0x9FD3FF;
    static final int NEGATIVE_OVERLAY_COLOR = 0xFFEE6677;
    static final long TITLE_SHOW_MILLIS = 3_000L;
    static final long TITLE_FULL_ALPHA_MILLIS = 2_000L;
    static final long DIMENSION_BLACKOUT_MILLIS = 500L;
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
        int blackoutAlpha = dimensionBlackoutAlpha(safeZoneState, nowMillis);
        if (blackoutAlpha > 0) {
            commands.add(HudRenderCommand.screenTint(HudRenderLayer.ZONE, HudTextHelper.withAlpha(0x000000, blackoutAlpha)));
        }

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
                    HudTextHelper.withAlpha(titleColor(safeZoneState), titleAlpha)
                ));
            }
        }

        String clippedOverlay = HudTextHelper.clipToWidth(persistentOverlayText(safeZoneState), maxWidth, widthMeasurer);
        if (!clippedOverlay.isEmpty()) {
            commands.add(HudRenderCommand.text(HudRenderLayer.ZONE, clippedOverlay, x, y, overlayColor(safeZoneState)));
        }
        return List.copyOf(commands);
    }

    static int dimensionBlackoutAlpha(ZoneState zoneState, long nowMillis) {
        ZoneState safeZoneState = zoneState == null ? ZoneState.empty() : zoneState;
        if (!safeZoneState.dimensionTransition()) {
            return 0;
        }
        long elapsed = Math.max(0L, Math.max(0L, nowMillis) - Math.max(0L, safeZoneState.changedAtMillis()));
        if (elapsed >= DIMENSION_BLACKOUT_MILLIS) {
            return 0;
        }
        long remaining = DIMENSION_BLACKOUT_MILLIS - elapsed;
        return HudTextHelper.clampAlpha((int) Math.round(220.0 * remaining / DIMENSION_BLACKOUT_MILLIS));
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
        return "— " + safeZoneState.zoneLabel() + negativeZoneText(safeZoneState) + " —";
    }

    static String persistentOverlayText(ZoneState zoneState) {
        ZoneState safeZoneState = zoneState == null ? ZoneState.empty() : zoneState;
        return "区域"
            + safeZoneState.zoneLabel()
            + statusText(safeZoneState)
            + negativeZoneText(safeZoneState)
            + " 灵气"
            + qiBar(safeZoneState.spiritQiNormalized())
            + " 危"
            + dangerText(safeZoneState.dangerLevel())
            + cadenceText(safeZoneState)
            + perceptionText(safeZoneState);
    }

    static String cadenceText(ZoneState zoneState) {
        ZoneState safeZoneState = zoneState == null ? ZoneState.empty() : zoneState;
        return safeZoneState.noCadence() ? " 节律无节律" : "";
    }

    static String statusText(ZoneState zoneState) {
        ZoneState safeZoneState = zoneState == null ? ZoneState.empty() : zoneState;
        return safeZoneState.collapsed() ? " 死域" : "";
    }

    static String negativeZoneText(ZoneState zoneState) {
        ZoneState safeZoneState = zoneState == null ? ZoneState.empty() : zoneState;
        return safeZoneState.negativeSpiritQi() ? " ⚠ 负灵域" : "";
    }

    static int titleColor(ZoneState zoneState) {
        ZoneState safeZoneState = zoneState == null ? ZoneState.empty() : zoneState;
        return safeZoneState.negativeSpiritQi() ? NEGATIVE_TITLE_COLOR : TITLE_COLOR;
    }

    static int overlayColor(ZoneState zoneState) {
        ZoneState safeZoneState = zoneState == null ? ZoneState.empty() : zoneState;
        return safeZoneState.negativeSpiritQi() ? NEGATIVE_OVERLAY_COLOR : OVERLAY_COLOR;
    }

    static String perceptionText(ZoneState zoneState) {
        ZoneState safeZoneState = zoneState == null ? ZoneState.empty() : zoneState;
        String text = safeZoneState.perceptionText();
        return text.isEmpty() ? "" : " " + text;
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
