package com.bong.client.visual;

import com.bong.client.hud.HudRenderCommand;
import com.bong.client.hud.HudRenderLayer;
import com.bong.client.hud.HudTextHelper;
import com.bong.client.state.VisualEffectState;

import java.util.List;

public final class VisualEffectPlanner {
    private static final int DEFAULT_SCREEN_WIDTH = 320;
    private static final int DEFAULT_SCREEN_HEIGHT = 180;
    private static final int WARNING_TEXT_Y_DIVISOR = 3;
    private static final int DECREE_TEXT_Y_DIVISOR = 4;

    private VisualEffectPlanner() {
    }

    public static List<HudRenderCommand> buildCommands(
        VisualEffectState visualEffectState,
        long nowMillis,
        HudTextHelper.WidthMeasurer widthMeasurer,
        int maxTextWidth,
        int screenWidth,
        int screenHeight,
        boolean enabled
    ) {
        VisualEffectState safeVisualEffectState = visualEffectState == null ? VisualEffectState.none() : visualEffectState;
        if (!enabled || safeVisualEffectState.isEmpty() || !safeVisualEffectState.isActiveAt(nowMillis)) {
            return List.of();
        }

        VisualEffectProfile profile = VisualEffectProfile.from(safeVisualEffectState);
        if (profile == null) {
            return List.of();
        }

        int alpha = alphaAt(safeVisualEffectState, nowMillis, profile);
        if (alpha <= 0) {
            return List.of();
        }

        return switch (profile) {
            case SYSTEM_WARNING -> buildWarningCommands(
                safeVisualEffectState,
                nowMillis,
                widthMeasurer,
                maxTextWidth,
                screenWidth,
                screenHeight,
                alpha,
                profile
            );
            case PERCEPTION -> List.of(HudRenderCommand.screenTint(
                HudRenderLayer.VISUAL,
                HudTextHelper.withAlpha(profile.baseColor(), alpha)
            ));
            case ERA_DECREE -> buildCenteredTextCommands(
                widthMeasurer,
                maxTextWidth,
                screenWidth,
                screenHeight,
                alpha,
                profile,
                0,
                DECREE_TEXT_Y_DIVISOR
            );
        };
    }

    static int alphaAt(VisualEffectState visualEffectState, long nowMillis, VisualEffectProfile profile) {
        return HudTextHelper.clampAlpha((int) Math.round(visualEffectState.scaledIntensityAt(nowMillis) * profile.maxAlpha()));
    }

    static int shakeOffset(VisualEffectState visualEffectState, long nowMillis) {
        long elapsedMillis = Math.max(0L, nowMillis - Math.max(0L, visualEffectState.startedAtMillis()));
        int amplitude = Math.max(1, (int) Math.round(visualEffectState.scaledIntensityAt(nowMillis) * 8.0));
        int reducedAmplitude = Math.max(1, amplitude / 2);
        return switch ((int) ((elapsedMillis / 75L) % 4L)) {
            case 0 -> amplitude;
            case 1 -> -amplitude;
            case 2 -> reducedAmplitude;
            default -> -reducedAmplitude;
        };
    }

    private static List<HudRenderCommand> buildWarningCommands(
        VisualEffectState visualEffectState,
        long nowMillis,
        HudTextHelper.WidthMeasurer widthMeasurer,
        int maxTextWidth,
        int screenWidth,
        int screenHeight,
        int alpha,
        VisualEffectProfile profile
    ) {
        return buildCenteredTextCommands(
            widthMeasurer,
            maxTextWidth,
            screenWidth,
            screenHeight,
            alpha,
            profile,
            shakeOffset(visualEffectState, nowMillis),
            WARNING_TEXT_Y_DIVISOR
        );
    }

    private static List<HudRenderCommand> buildCenteredTextCommands(
        HudTextHelper.WidthMeasurer widthMeasurer,
        int maxTextWidth,
        int screenWidth,
        int screenHeight,
        int alpha,
        VisualEffectProfile profile,
        int xOffset,
        int yDivisor
    ) {
        if (widthMeasurer == null || profile.overlayLabel() == null || profile.overlayLabel().isBlank()) {
            return List.of();
        }

        int resolvedScreenWidth = normalizeScreenWidth(screenWidth);
        int resolvedScreenHeight = normalizeScreenHeight(screenHeight);
        int resolvedMaxTextWidth = normalizeMaxTextWidth(maxTextWidth, resolvedScreenWidth);
        String clippedLabel = HudTextHelper.clipToWidth(profile.overlayLabel(), resolvedMaxTextWidth, widthMeasurer);
        if (clippedLabel.isEmpty()) {
            return List.of();
        }

        int textWidth = Math.max(0, widthMeasurer.measure(clippedLabel));
        int centeredX = Math.max(0, (resolvedScreenWidth - textWidth) / 2);
        int maxX = Math.max(0, resolvedScreenWidth - textWidth);
        int x = Math.max(0, Math.min(maxX, centeredX + xOffset));
        int y = Math.max(18, resolvedScreenHeight / Math.max(1, yDivisor));
        return List.of(HudRenderCommand.text(
            HudRenderLayer.VISUAL,
            clippedLabel,
            x,
            y,
            HudTextHelper.withAlpha(profile.baseColor(), alpha)
        ));
    }

    private static int normalizeScreenWidth(int screenWidth) {
        return screenWidth > 0 ? screenWidth : DEFAULT_SCREEN_WIDTH;
    }

    private static int normalizeScreenHeight(int screenHeight) {
        return screenHeight > 0 ? screenHeight : DEFAULT_SCREEN_HEIGHT;
    }

    private static int normalizeMaxTextWidth(int maxTextWidth, int screenWidth) {
        if (maxTextWidth > 0) {
            return maxTextWidth;
        }
        return Math.max(80, screenWidth - 24);
    }
}
