package com.bong.client;

import com.bong.client.hud.BongHudOrchestrator;
import com.bong.client.hud.BongHudStateStore;
import com.bong.client.hud.BongToast;
import com.bong.client.hud.HudRenderCommand;
import com.bong.client.visual.EdgeDecalRenderer;
import com.bong.client.visual.OverlayQuadRenderer;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.DrawContext;

import java.util.List;
import java.util.Objects;

public class BongHud {
    private static final int HUD_TEXT_MAX_WIDTH = 220;
    static final String BASELINE_STATUS_TEXT = BongHudOrchestrator.BASELINE_LABEL;
    static final int BASELINE_TEXT_COLOR = 0xFFFFFF;
    private static final int BASELINE_X = 10;
    private static final int BASELINE_Y = 10;
    private static final int TOAST_BACKGROUND_COLOR = 0x88000000;
    private static final int TOAST_HORIZONTAL_PADDING = 4;
    private static final int TOAST_VERTICAL_PADDING = 4;

    public static void render(DrawContext context, float tickDelta) {
        MinecraftClient client = MinecraftClient.getInstance();
        long nowMillis = System.currentTimeMillis();
        List<HudRenderCommand> commands = BongHudOrchestrator.buildCommands(
            BongHudStateStore.snapshot(),
            nowMillis,
            client.textRenderer::getWidth,
            HUD_TEXT_MAX_WIDTH,
            client.getWindow().getScaledWidth(),
            client.getWindow().getScaledHeight()
        );

        for (HudRenderCommand command : commands) {
            if (command.isText()) {
                context.drawTextWithShadow(client.textRenderer, command.text(), command.x(), command.y(), command.color());
                continue;
            }

            if (command.isToast()) {
                BongToast.render(
                    context,
                    client.textRenderer,
                    client.getWindow().getScaledWidth(),
                    client.getWindow().getScaledHeight(),
                    command
                );
            }
        }

        int scaledWidth = client.getWindow().getScaledWidth();
        int scaledHeight = client.getWindow().getScaledHeight();
        for (HudRenderCommand command : commands) {
            if (command.isScreenTint()) {
                OverlayQuadRenderer.render(context, scaledWidth, scaledHeight, command.color());
            } else if (command.isEdgeVignette()) {
                EdgeDecalRenderer.render(context, scaledWidth, scaledHeight, command.color());
            }
        }
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

        surface.drawTextWithShadow(snapshot.baselineText(), BASELINE_X, BASELINE_Y, BASELINE_TEXT_COLOR);
        BongZoneHud.render(surface, snapshot.zone(), snapshot.nowMs());
        BongEventAlertOverlay.render(surface, snapshot.eventAlert());
        renderToast(surface, snapshot.toast());
    }

    private static void renderToast(HudSurface surface, NarrationState.ToastState toast) {
        if (toast == null || toast.text().isBlank()) {
            return;
        }

        int width = surface.measureText(toast.text());
        int x = (surface.windowWidth() - width) / 2;
        int y = surface.windowHeight() / 4;
        surface.fill(
            x - TOAST_HORIZONTAL_PADDING,
            y - TOAST_VERTICAL_PADDING,
            x + width + TOAST_HORIZONTAL_PADDING,
            y + 12,
            TOAST_BACKGROUND_COLOR
        );
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

    record HudSnapshot(
        String baselineText,
        NarrationState.ToastState toast,
        ZoneState.ZoneHudState zone,
        EventAlertState.BannerState eventAlert,
        long nowMs
    ) {
        HudSnapshot {
            Objects.requireNonNull(baselineText, "baselineText");
        }
    }
}
