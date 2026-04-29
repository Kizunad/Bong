package com.bong.client.hud;

import com.bong.client.combat.store.TribulationBroadcastStore;

import java.util.ArrayList;
import java.util.List;

/**
 * Top-of-screen red broadcast + spectate tip (plan §U6).
 */
public final class TribulationBroadcastHudPlanner {
    public static final int TOP_MARGIN = 28;
    public static final int BAR_HEIGHT = 18;
    public static final int TEXT_COLOR = 0xFFFF4040;
    public static final int BG_COLOR = 0xC0200000;
    public static final int SPECTATE_COLOR = 0xFFFFE080;
    public static final double SPECTATE_HINT_DISTANCE = 50.0;

    private TribulationBroadcastHudPlanner() {}

    public static List<HudRenderCommand> buildCommands(
        int screenWidth,
        int screenHeight,
        long nowMs
    ) {
        List<HudRenderCommand> out = new ArrayList<>();
        TribulationBroadcastStore.State state = TribulationBroadcastStore.snapshot();
        if (!state.active() || state.expired(nowMs)) return out;
        if (screenWidth <= 0 || screenHeight <= 0) return out;

        String stageLabel = switch (state.stage()) {
            case "warn" -> "\u5929\u52ab\u5c06\u81f3";
            case "locked" -> "\u52ab\u9501\u5df2\u6210";
            case "striking" -> "\u5929\u96f7\u964d\u4e34";
            case "done" -> "\u5929\u52ab\u5df2\u8fc7";
            default -> "\u5929\u52ab\u5f02\u52a8";
        };
        String line = "\u26a1 " + stageLabel
            + " \u00b7 " + (state.actorName().isEmpty() ? "\u65e0\u540d\u4fee\u58eb" : state.actorName())
            + " \u00b7 \u5750\u6807 (" + Math.round(state.worldX()) + ", " + Math.round(state.worldZ()) + ")";
        if (state.spectateDistance() >= 0d) {
            line += " \u00b7 \u8ddd\u79bb " + Math.round(state.spectateDistance()) + " \u683c";
        }

        // Background bar
        out.add(HudRenderCommand.rect(HudRenderLayer.TRIBULATION, 0, TOP_MARGIN, screenWidth, BAR_HEIGHT, BG_COLOR));
        // Approximate centering: text width ~= line.length() * 6 (ASCII fallback).
        int approxWidth = Math.max(line.length() * 6, 120);
        int x = Math.max(4, (screenWidth - approxWidth) / 2);
        out.add(HudRenderCommand.text(HudRenderLayer.TRIBULATION, line, x, TOP_MARGIN + 5, TEXT_COLOR));

        // Spectate hint
        if (state.spectateInvite() && state.spectateDistance() >= 0d
            && state.spectateDistance() <= SPECTATE_HINT_DISTANCE) {
            String hint = "(" + Math.round(state.spectateDistance()) + " \u683c\u5185\uff0c\u53ef\u524d\u5f80\u89c2\u6218\uff0c100 \u683c\u5185\u4f1a\u627f\u96f7)";
            out.add(HudRenderCommand.text(
                HudRenderLayer.TRIBULATION, hint, x, TOP_MARGIN + BAR_HEIGHT + 2, SPECTATE_COLOR
            ));
        }
        return out;
    }
}
