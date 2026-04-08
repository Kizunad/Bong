package com.bong.client;

import net.minecraft.client.MinecraftClient;
import net.minecraft.client.font.TextRenderer;
import net.minecraft.client.gui.DrawContext;

import java.util.Objects;

public final class BongHud {
    static final String BASELINE_STATUS_TEXT = "Bong Client Connected";
    static final int BASELINE_TEXT_COLOR = 0xFFFFFF;
    private static final int TOAST_BACKGROUND_COLOR = 0x88000000;

    private BongHud() {
    }

    public static void render(DrawContext context, float tickDelta) {
        MinecraftClient client = MinecraftClient.getInstance();
        if (client == null || client.textRenderer == null) {
            return;
        }

        renderSurface(new DrawContextHudSurface(context, client.textRenderer), snapshot(System.currentTimeMillis()));
    }

    static HudSnapshot snapshot(long nowMs) {
        return new HudSnapshot(
                BASELINE_STATUS_TEXT,
                NarrationState.getCurrentToast(nowMs),
                ZoneState.getCurrentZone(),
                EventAlertState.getCurrentBanner(nowMs),
                nowMs
        );
    }

    static void renderSurface(HudSurface surface, HudSnapshot snapshot) {
        Objects.requireNonNull(surface, "surface");
        Objects.requireNonNull(snapshot, "snapshot");

        surface.drawTextWithShadow(snapshot.baselineText(), 10, 10, BASELINE_TEXT_COLOR);

        BongZoneHud.render(surface, snapshot.zone(), snapshot.nowMs());

        BongEventAlertOverlay.render(surface, snapshot.eventAlert());

        NarrationState.ToastState toast = snapshot.toast();
        if (toast == null) {
            return;
        }

        int width = surface.measureText(toast.text());
        int x = (surface.windowWidth() - width) / 2;
        int y = surface.windowHeight() / 4;

        surface.fill(x - 4, y - 4, x + width + 4, y + 12, TOAST_BACKGROUND_COLOR);
        surface.drawText(toast.text(), x, y, toast.color(), true);
    }

    interface HudSurface {
        int windowWidth();

        int windowHeight();

        int measureText(String text);

        void fill(int x1, int y1, int x2, int y2, int color);

        void drawTextWithShadow(String text, int x, int y, int color);

        void drawText(String text, int x, int y, int color, boolean shadow);
    }

    record HudSnapshot(String baselineText, NarrationState.ToastState toast, ZoneState.ZoneHudState zone,
                       EventAlertState.BannerState eventAlert, long nowMs) {
        HudSnapshot {
            Objects.requireNonNull(baselineText, "baselineText");
        }
    }

    private record DrawContextHudSurface(DrawContext context, TextRenderer textRenderer) implements HudSurface {
        private DrawContextHudSurface {
            Objects.requireNonNull(context, "context");
            Objects.requireNonNull(textRenderer, "textRenderer");
        }

        @Override
        public int windowWidth() {
            return context.getScaledWindowWidth();
        }

        @Override
        public int windowHeight() {
            return context.getScaledWindowHeight();
        }

        @Override
        public int measureText(String text) {
            return textRenderer.getWidth(Objects.requireNonNull(text, "text"));
        }

        @Override
        public void fill(int x1, int y1, int x2, int y2, int color) {
            context.fill(x1, y1, x2, y2, color);
        }

        @Override
        public void drawTextWithShadow(String text, int x, int y, int color) {
            context.drawTextWithShadow(textRenderer, text, x, y, color);
        }

        @Override
        public void drawText(String text, int x, int y, int color, boolean shadow) {
            context.drawText(textRenderer, text, x, y, color, shadow);
        }
    }
}
