package com.bong.client.ui;

import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.DrawContext;

public final class ScreenTransitionOverlay {
    private static final int BASE_COLOR = 0xFF101318;
    private static final int FOG_COLOR = 0xFFB8B0A0;
    private static final int VIGNETTE_COLOR = 0xFF050508;
    private static final int PURPLE_COLOR = 0xFF503070;

    private ScreenTransitionOverlay() {
    }

    public static void render(DrawContext context, MinecraftClient client, long nowMs) {
        if (context == null || client == null || client.getWindow() == null) {
            return;
        }
        render(context, client.getWindow().getScaledWidth(), client.getWindow().getScaledHeight(), nowMs);
    }

    public static void render(DrawContext context, int width, int height, long nowMs) {
        ScreenTransitionController.ActiveTransition active = ScreenTransitionController.activeTransition();
        if (context == null || active == null || width <= 0 || height <= 0) {
            return;
        }

        ScreenTransition.Frame frame = active.handle().sample(nowMs, width, height);
        int alpha = (int) Math.round(112.0 * (1.0 - Math.abs(frame.progress() - 0.5) * 0.7));
        context.fill(0, 0, width, height, withAlpha(BASE_COLOR, alpha));

        switch (active.handle().type()) {
            case SLIDE_UP -> context.fill(0, frame.offsetY(), width, height, withAlpha(BASE_COLOR, 120));
            case SLIDE_DOWN -> context.fill(0, 0, width, Math.max(0, frame.offsetY()), withAlpha(BASE_COLOR, 90));
            case SLIDE_RIGHT -> context.fill(frame.offsetX(), 0, width, height, withAlpha(BASE_COLOR, 100));
            case SLIDE_LEFT -> context.fill(0, 0, Math.max(0, width + frame.offsetX()), height, withAlpha(BASE_COLOR, 90));
            case SCALE_UP, SCALE_DOWN -> drawScaleFocus(context, width, height, frame.scale());
            default -> {
            }
        }

        drawOverlayStyle(context, width, height, active.spec().overlayStyle(), frame.progress());
    }

    private static void drawScaleFocus(DrawContext context, int width, int height, double scale) {
        int focusW = (int) Math.round(width * Math.max(0.0, Math.min(1.0, scale)));
        int focusH = (int) Math.round(height * Math.max(0.0, Math.min(1.0, scale)));
        int x0 = (width - focusW) / 2;
        int y0 = (height - focusH) / 2;
        context.fill(x0, y0, x0 + focusW, y0 + focusH, withAlpha(0xFF1A1713, 64));
        context.fill(x0, y0, x0 + focusW, y0 + 1, withAlpha(0xFFE8D8A0, 110));
        context.fill(x0, y0 + focusH - 1, x0 + focusW, y0 + focusH, withAlpha(0xFFE8D8A0, 110));
        context.fill(x0, y0, x0 + 1, y0 + focusH, withAlpha(0xFFE8D8A0, 110));
        context.fill(x0 + focusW - 1, y0, x0 + focusW, y0 + focusH, withAlpha(0xFFE8D8A0, 110));
    }

    private static void drawOverlayStyle(
        DrawContext context,
        int width,
        int height,
        TransitionConfig.OverlayStyle style,
        double progress
    ) {
        double pulse = 1.0 - Math.abs(0.5 - progress) * 2.0;
        switch (style) {
            case FOG -> context.fill(0, 0, width, height, withAlpha(FOG_COLOR, (int) Math.round(13.0 * pulse)));
            case VIGNETTE -> {
                int alpha = (int) Math.round(72.0 * pulse);
                context.fill(0, 0, width, 18, withAlpha(VIGNETTE_COLOR, alpha));
                context.fill(0, height - 18, width, height, withAlpha(VIGNETTE_COLOR, alpha));
                context.fill(0, 0, 18, height, withAlpha(VIGNETTE_COLOR, alpha));
                context.fill(width - 18, 0, width, height, withAlpha(VIGNETTE_COLOR, alpha));
            }
            case PURPLE_TINT -> context.fill(0, 0, width, height, withAlpha(PURPLE_COLOR, (int) Math.round(28.0 * pulse)));
            case NONE -> {
            }
        }
    }

    static int withAlpha(int rgbOrArgb, int alpha) {
        int clamped = Math.max(0, Math.min(255, alpha));
        return (clamped << 24) | (rgbOrArgb & 0x00FFFFFF);
    }
}
