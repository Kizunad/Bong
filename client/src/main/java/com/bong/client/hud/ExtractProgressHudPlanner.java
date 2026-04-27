package com.bong.client.hud;

import com.bong.client.tsy.ExtractState;
import com.bong.client.tsy.ExtractStateStore;
import com.bong.client.tsy.RiftPortalView;
import net.minecraft.client.MinecraftClient;

import java.util.ArrayList;
import java.util.List;

public final class ExtractProgressHudPlanner {
    private static final int PANEL_WIDTH = 240;
    private static final int PANEL_HEIGHT = 44;
    private static final int TRACK_HEIGHT = 5;
    private static final int BG = 0xD0111118;
    private static final int BORDER = 0xFF60A8FF;
    private static final int DANGER = 0xFFFF5050;
    private static final int TEXT = 0xFFE6F3FF;
    private static final int MUTED = 0xFF8EA5B8;
    private static final int FILL = 0xFF60D8FF;

    private ExtractProgressHudPlanner() {
    }

    public static List<HudRenderCommand> buildCommands(
        ExtractState state,
        HudTextHelper.WidthMeasurer widthMeasurer,
        int screenWidth,
        int screenHeight,
        long nowMs
    ) {
        List<HudRenderCommand> out = new ArrayList<>();
        if (screenWidth <= 0 || screenHeight <= 0 || widthMeasurer == null) {
            return out;
        }
        ExtractState safe = state == null ? ExtractState.empty() : state;
        if (safe.screenFlashActive(nowMs)) {
            out.add(HudRenderCommand.screenTint(HudRenderLayer.TSY_EXTRACT, safe.screenFlashColor()));
        }
        appendCollapse(out, safe, screenWidth, nowMs);
        if (safe.extracting()) {
            appendExtractBar(out, safe, widthMeasurer, screenWidth, screenHeight);
        } else {
            appendNearestPortalHint(out, safe, widthMeasurer, screenWidth, screenHeight);
        }
        if (safe.hasTimedMessage(nowMs)) {
            appendMessage(out, safe, widthMeasurer, screenWidth, screenHeight);
        }
        return List.copyOf(out);
    }

    private static void appendExtractBar(
        List<HudRenderCommand> out,
        ExtractState state,
        HudTextHelper.WidthMeasurer widthMeasurer,
        int screenWidth,
        int screenHeight
    ) {
        int x = (screenWidth - PANEL_WIDTH) / 2;
        int y = screenHeight - 92;
        appendPanel(out, x, y, BORDER);
        double progress = state.requiredTicks() <= 0 ? 0.0 : (double) state.elapsedTicks() / (double) state.requiredTicks();
        progress = Math.max(0.0, Math.min(1.0, progress));
        int remainingTicks = Math.max(0, state.requiredTicks() - state.elapsedTicks());
        String label = "撤离中 " + secondsLabel(remainingTicks) + " · " + kindLabel(state.activePortalKind());
        out.add(HudRenderCommand.text(HudRenderLayer.TSY_EXTRACT, clip(label, PANEL_WIDTH - 16, widthMeasurer), x + 8, y + 8, TEXT));
        int trackX = x + 8;
        int trackY = y + 28;
        int trackW = PANEL_WIDTH - 16;
        out.add(HudRenderCommand.rect(HudRenderLayer.TSY_EXTRACT, trackX, trackY, trackW, TRACK_HEIGHT, 0xFF101820));
        out.add(HudRenderCommand.rect(HudRenderLayer.TSY_EXTRACT, trackX, trackY, (int) Math.round(trackW * progress), TRACK_HEIGHT, FILL));
    }

    private static void appendNearestPortalHint(
        List<HudRenderCommand> out,
        ExtractState state,
        HudTextHelper.WidthMeasurer widthMeasurer,
        int screenWidth,
        int screenHeight
    ) {
        MinecraftClient client = MinecraftClient.getInstance();
        RiftPortalView portal = ExtractStateStore.nearestPortal(client == null ? null : client.player);
        if (portal == null) {
            return;
        }
        int x = (screenWidth - PANEL_WIDTH) / 2;
        int y = screenHeight - 78;
        appendPanel(out, x, y, BORDER);
        String label = kindLabel(portal.kind()) + " · 按 Y 开始撤离 [" + secondsLabel(portal.currentExtractTicks()) + "]";
        out.add(HudRenderCommand.text(HudRenderLayer.TSY_EXTRACT, clip(label, PANEL_WIDTH - 16, widthMeasurer), x + 8, y + 12, TEXT));
        out.add(HudRenderCommand.text(HudRenderLayer.TSY_EXTRACT, "移动 / 战斗 / 受击会归零", x + 8, y + 26, MUTED));
    }

    private static void appendCollapse(List<HudRenderCommand> out, ExtractState state, int screenWidth, long nowMs) {
        if (!state.collapseActive(nowMs)) {
            return;
        }
        int remaining = state.collapseRemainingTicks(nowMs);
        out.add(HudRenderCommand.screenTint(HudRenderLayer.TSY_EXTRACT, 0x22FF0000));
        String label = "坍缩倒计时 " + secondsLabel(remaining);
        out.add(HudRenderCommand.text(HudRenderLayer.TSY_EXTRACT, label, Math.max(8, screenWidth / 2 - 48), 24, DANGER));
    }

    private static void appendMessage(
        List<HudRenderCommand> out,
        ExtractState state,
        HudTextHelper.WidthMeasurer widthMeasurer,
        int screenWidth,
        int screenHeight
    ) {
        String text = clip(state.message(), PANEL_WIDTH, widthMeasurer);
        int x = (screenWidth - widthMeasurer.measure(text)) / 2;
        int y = screenHeight - 124;
        out.add(HudRenderCommand.text(HudRenderLayer.TSY_EXTRACT, text, x, y, state.messageColor()));
    }

    private static void appendPanel(List<HudRenderCommand> out, int x, int y, int border) {
        out.add(HudRenderCommand.rect(HudRenderLayer.TSY_EXTRACT, x + 2, y + 2, PANEL_WIDTH, PANEL_HEIGHT, 0x88000000));
        out.add(HudRenderCommand.rect(HudRenderLayer.TSY_EXTRACT, x, y, PANEL_WIDTH, PANEL_HEIGHT, BG));
        out.add(HudRenderCommand.rect(HudRenderLayer.TSY_EXTRACT, x, y, PANEL_WIDTH, 1, border));
        out.add(HudRenderCommand.rect(HudRenderLayer.TSY_EXTRACT, x, y + PANEL_HEIGHT - 1, PANEL_WIDTH, 1, border));
        out.add(HudRenderCommand.rect(HudRenderLayer.TSY_EXTRACT, x, y, 1, PANEL_HEIGHT, border));
        out.add(HudRenderCommand.rect(HudRenderLayer.TSY_EXTRACT, x + PANEL_WIDTH - 1, y, 1, PANEL_HEIGHT, border));
    }

    private static String clip(String value, int maxWidth, HudTextHelper.WidthMeasurer widthMeasurer) {
        return HudTextHelper.clipToWidth(value == null ? "" : value, maxWidth, widthMeasurer);
    }

    private static String secondsLabel(int ticks) {
        double seconds = Math.max(0, ticks) / 20.0;
        if (seconds >= 10.0 || Math.abs(seconds - Math.rint(seconds)) < 0.05) {
            return Math.round(seconds) + "s";
        }
        return String.format(java.util.Locale.ROOT, "%.1fs", seconds);
    }

    private static String kindLabel(String kind) {
        return switch (kind == null ? "" : kind) {
            case "main_rift" -> "主裂缝";
            case "deep_rift" -> "深层缝";
            case "collapse_tear" -> "塌缩裂口";
            default -> "撤离点";
        };
    }
}
