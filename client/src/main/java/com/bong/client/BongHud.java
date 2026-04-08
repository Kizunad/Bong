package com.bong.client;

import net.minecraft.client.MinecraftClient;
import net.minecraft.client.font.TextRenderer;
import net.minecraft.client.gui.DrawContext;

public class BongHud {
    private static final int TOAST_BACKGROUND_COLOR = 0x88000000;
    private static final int TOAST_PADDING_X = 6;
    private static final int TOAST_PADDING_Y = 4;
    private static final int TOAST_VERTICAL_DIVISOR = 4;
    private static final int TOAST_MIN_TOP = 16;
    private static final int HUD_TEXT_COLOR = 0xFFFFFF;
    private static final int EVENT_ALERT_COLOR = 0xFF5555;
    private static final int ENTRY_BANNER_COLOR = 0xFFD700;
    private static final int HUD_LEFT = 6;
    private static final int ZONE_LINE_Y = 20;
    private static final int DANGER_LINE_Y = 32;
    private static final int EVENT_LINE_Y = 44;
    private static final int EVENT_ALERT_BOTTOM_MARGIN = 40;

    public static void render(DrawContext context, float tickDelta) {
        MinecraftClient client = MinecraftClient.getInstance();
        TextRenderer textRenderer = client.textRenderer;
        int scaledWidth = client.getWindow().getScaledWidth();
        int scaledHeight = client.getWindow().getScaledHeight();

        renderZoneHud(context, textRenderer, scaledWidth, scaledHeight);
        renderEventAlertOverlay(context, textRenderer, scaledWidth, scaledHeight);
        renderNarrationToast(context, textRenderer, scaledWidth, scaledHeight);
    }

    private static void renderNarrationToast(
        DrawContext context,
        TextRenderer textRenderer,
        int scaledWidth,
        int scaledHeight
    ) {
        NarrationToastState.ActiveToast activeToast = NarrationToastState.peek();
        if (activeToast == null || activeToast.text().isBlank()) {
            return;
        }

        String toastText = activeToast.text();

        int textWidth = textRenderer.getWidth(toastText);
        int x = (scaledWidth - textWidth) / 2;
        int y = Math.max(TOAST_MIN_TOP, scaledHeight / TOAST_VERTICAL_DIVISOR);
        int left = x - TOAST_PADDING_X;
        int top = y - TOAST_PADDING_Y;
        int right = left + textWidth + (TOAST_PADDING_X * 2);
        int bottom = top + textRenderer.fontHeight + (TOAST_PADDING_Y * 2);

        context.fill(left, top, right, bottom, TOAST_BACKGROUND_COLOR);
        context.drawTextWithShadow(textRenderer, toastText, x, y, activeToast.color());
    }

    private static void renderZoneHud(
        DrawContext context,
        TextRenderer textRenderer,
        int scaledWidth,
        int scaledHeight
    ) {
        ZoneHudState.ZoneSnapshot zoneSnapshot = ZoneHudState.peek();
        if (zoneSnapshot == null) {
            return;
        }

        String qiText = String.format("%.2f", zoneSnapshot.spiritQi());
        context.drawTextWithShadow(
            textRenderer,
            "区域: " + zoneSnapshot.zone() + "  灵气: " + qiText,
            HUD_LEFT,
            ZONE_LINE_Y,
            HUD_TEXT_COLOR
        );
        context.drawTextWithShadow(
            textRenderer,
            "危险等级: " + zoneSnapshot.dangerLevel(),
            HUD_LEFT,
            DANGER_LINE_Y,
            HUD_TEXT_COLOR
        );
        if (!zoneSnapshot.activeEvents().isEmpty()) {
            context.drawTextWithShadow(
                textRenderer,
                "活跃事件: " + String.join(", ", zoneSnapshot.activeEvents()),
                HUD_LEFT,
                EVENT_LINE_Y,
                HUD_TEXT_COLOR
            );
        }

        if (ZoneHudState.shouldShowEntryBanner(System.currentTimeMillis())) {
            String bannerText = "— " + zoneSnapshot.zone() + " —";
            int bannerWidth = textRenderer.getWidth(bannerText);
            int bannerX = (scaledWidth - bannerWidth) / 2;
            int bannerY = scaledHeight / 3;
            context.drawTextWithShadow(textRenderer, bannerText, bannerX, bannerY, ENTRY_BANNER_COLOR);
        }
    }

    private static void renderEventAlertOverlay(
        DrawContext context,
        TextRenderer textRenderer,
        int scaledWidth,
        int scaledHeight
    ) {
        EventAlertState.ActiveAlert alert = EventAlertState.peek();
        if (alert == null || alert.message().isBlank()) {
            return;
        }

        String message = alert.message();
        int textWidth = textRenderer.getWidth(message);
        int x = (scaledWidth - textWidth) / 2;
        int y = scaledHeight - EVENT_ALERT_BOTTOM_MARGIN;
        int left = x - TOAST_PADDING_X;
        int top = y - TOAST_PADDING_Y;
        int right = left + textWidth + (TOAST_PADDING_X * 2);
        int bottom = top + textRenderer.fontHeight + (TOAST_PADDING_Y * 2);

        context.fill(left, top, right, bottom, TOAST_BACKGROUND_COLOR);
        context.drawTextWithShadow(textRenderer, message, x, y, EVENT_ALERT_COLOR);
    }
}
