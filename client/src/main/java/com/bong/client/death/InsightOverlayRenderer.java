package com.bong.client.death;

import com.bong.client.hud.HudRenderCommand;
import com.bong.client.hud.HudRenderLayer;

import java.util.ArrayList;
import java.util.List;

public final class InsightOverlayRenderer {
    private InsightOverlayRenderer() {}

    public static int visibleLineCount(DeathCinematicState state) {
        if (state == null || !state.active() || state.insightText().isEmpty()) return 0;
        double lineBudget = state.phaseProgress() * state.insightText().size();
        return Math.max(1, Math.min(state.insightText().size(), (int) Math.ceil(lineBudget)));
    }

    public static List<HudRenderCommand> buildCommands(DeathCinematicState state, int width, int height) {
        if (state == null || !state.active() || width <= 0 || height <= 0) return List.of();
        List<HudRenderCommand> out = new ArrayList<>();
        out.add(HudRenderCommand.screenTint(HudRenderLayer.VISUAL, 0xD8000000));
        int visible = visibleLineCount(state);
        int startX = width / 2 + Math.min(110, width / 4);
        int top = Math.max(28, height / 2 - 70);
        for (int i = 0; i < visible; i++) {
            String line = state.insightText().get(i);
            int color = line.contains("运数") || line.contains("坍缩渊") ? 0xFFE07070 : 0xFFC0B090;
            out.add(HudRenderCommand.text(
                HudRenderLayer.VISUAL,
                verticalPreview(line),
                startX - i * 28,
                top,
                color
            ));
        }
        return out;
    }

    static String verticalPreview(String line) {
        if (line == null || line.isBlank()) return "";
        String compact = line.replace(" ", "");
        return compact.length() <= 12 ? compact : compact.substring(0, 12);
    }
}
