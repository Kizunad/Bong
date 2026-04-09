package com.bong.client.hud;

import com.bong.client.BongClientFeatures;

import java.util.ArrayList;
import java.util.List;

public final class BongHudOrchestrator {
    public static final String BASELINE_LABEL = "Bong Client Connected";

    private static final int BASELINE_X = 10;
    private static final int BASELINE_Y = 10;
    private static final int LINE_HEIGHT = 12;
    private static final int DEFAULT_TEXT_WIDTH = 220;

    private BongHudOrchestrator() {
    }

    public static List<HudRenderCommand> buildCommands(
        BongHudStateSnapshot snapshot,
        long nowMillis,
        HudTextHelper.WidthMeasurer widthMeasurer,
        int maxTextWidth
    ) {
        return buildCommands(snapshot, nowMillis, widthMeasurer, maxTextWidth, 0, 0);
    }

    public static List<HudRenderCommand> buildCommands(
        BongHudStateSnapshot snapshot,
        long nowMillis,
        HudTextHelper.WidthMeasurer widthMeasurer,
        int maxTextWidth,
        int screenWidth,
        int screenHeight
    ) {
        BongHudStateSnapshot safeSnapshot = snapshot == null ? BongHudStateSnapshot.empty() : snapshot;
        int normalizedWidth = normalizeWidth(maxTextWidth);
        List<HudRenderCommand> commands = new ArrayList<>();
        commands.add(HudRenderCommand.text(HudRenderLayer.BASELINE, BASELINE_LABEL, BASELINE_X, BASELINE_Y, 0xFFFFFF));

        int nextY = BASELINE_Y + LINE_HEIGHT;
        if (ZoneHudRenderer.append(
            commands,
            safeSnapshot.zoneState(),
            nowMillis,
            widthMeasurer,
            normalizedWidth,
            BASELINE_X,
            nextY,
            screenWidth,
            screenHeight
        )) {
            nextY += LINE_HEIGHT;
        }

        if (BongClientFeatures.ENABLE_TOASTS
            && ToastHudRenderer.append(commands, nowMillis, widthMeasurer, normalizedWidth, BASELINE_X, nextY)) {
            nextY += LINE_HEIGHT;
        }

        if (BongClientFeatures.ENABLE_VISUAL_EFFECTS) {
            VisualHudRenderer.append(
                commands,
                safeSnapshot.visualEffectState(),
                nowMillis,
                widthMeasurer,
                normalizedWidth,
                screenWidth,
                screenHeight
            );
        }

        return List.copyOf(commands);
    }

    private static int normalizeWidth(int requestedWidth) {
        return requestedWidth > 0 ? requestedWidth : DEFAULT_TEXT_WIDTH;
    }
}
