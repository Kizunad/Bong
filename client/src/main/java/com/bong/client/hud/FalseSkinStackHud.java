package com.bong.client.hud;

import com.bong.client.combat.store.FalseSkinHudStateStore;

import java.util.ArrayList;
import java.util.List;

/** Compact tuike-v2 stacked false-skin HUD. */
public final class FalseSkinStackHud {
    static final int PANEL_W = 56;
    static final int PANEL_H = 26;
    static final int LAYER_W = 14;
    static final int LAYER_H = 16;
    static final int GAP = 2;

    private static final int BG_COLOR = 0xA0100E0A;
    private static final int TRACK_COLOR = 0xCC241C16;
    private static final int QUALITY_COLOR = 0xFFE0C080;
    private static final int ANCIENT_COLOR = 0xFFBFD8FF;
    private static final int SKIN_EDGE_COLOR = 0xFF8A6A42;

    private FalseSkinStackHud() {
    }

    public static List<HudRenderCommand> buildCommands(
        FalseSkinHudStateStore.State state,
        int screenWidth,
        int screenHeight
    ) {
        FalseSkinHudStateStore.State safeState = state == null ? FalseSkinHudStateStore.State.NONE : state;
        if (!safeState.active() || screenWidth <= 0 || screenHeight <= 0) return List.of();

        int x = MiniBodyHudPlanner.MARGIN_X + MiniBodyHudPlanner.PANEL_W + 4;
        int y = screenHeight - MiniBodyHudPlanner.PANEL_H - MiniBodyHudPlanner.MARGIN_Y;
        List<HudRenderCommand> out = new ArrayList<>();
        out.add(HudRenderCommand.rect(HudRenderLayer.DERIVED_ATTR, x, y, PANEL_W, PANEL_H, BG_COLOR));
        out.add(HudRenderCommand.text(HudRenderLayer.DERIVED_ATTR, "伪皮", x + 3, y + 2, QUALITY_COLOR));

        int maxLayers = Math.min(3, safeState.layers().size());
        int startX = x + 4;
        int layerY = y + 10;
        for (int i = 0; i < maxLayers; i++) {
            FalseSkinHudStateStore.Layer layer = safeState.layers().get(i);
            int layerX = startX + i * (LAYER_W + GAP);
            appendLayer(out, layer, layerX, layerY);
        }
        return List.copyOf(out);
    }

    private static void appendLayer(
        List<HudRenderCommand> out,
        FalseSkinHudStateStore.Layer layer,
        int x,
        int y
    ) {
        int edgeColor = "ancient".equals(layer.tier()) ? ANCIENT_COLOR : SKIN_EDGE_COLOR;
        out.add(HudRenderCommand.rect(HudRenderLayer.DERIVED_ATTR, x, y, LAYER_W, LAYER_H, TRACK_COLOR));
        out.add(HudRenderCommand.rect(HudRenderLayer.DERIVED_ATTR, x, y, LAYER_W, 1, edgeColor));
        out.add(HudRenderCommand.rect(HudRenderLayer.DERIVED_ATTR, x, y + LAYER_H - 1, LAYER_W, 1, edgeColor));
        out.add(HudRenderCommand.rect(HudRenderLayer.DERIVED_ATTR, x, y, 1, LAYER_H, edgeColor));
        out.add(HudRenderCommand.rect(HudRenderLayer.DERIVED_ATTR, x + LAYER_W - 1, y, 1, LAYER_H, edgeColor));

        int fillH = Math.max(1, Math.round((LAYER_H - 4) * Math.min(1f, layer.spiritQuality() / 3f)));
        out.add(HudRenderCommand.rect(
            HudRenderLayer.DERIVED_ATTR,
            x + 3,
            y + LAYER_H - 2 - fillH,
            LAYER_W - 6,
            fillH,
            "ancient".equals(layer.tier()) ? ANCIENT_COLOR : QUALITY_COLOR
        ));
    }
}
