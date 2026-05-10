package com.bong.client.hud;

import com.bong.client.combat.store.FalseSkinHudStateStore;

import java.util.ArrayList;
import java.util.List;
import java.util.Locale;

/** Shows current contamination carried by the outer false skin. */
public final class ContamLoadHud {
    static final int PANEL_W = 56;
    static final int PANEL_H = 16;

    private static final int BG_COLOR = 0xA0110A16;
    private static final int TRACK_COLOR = 0xCC201624;
    private static final int LOAD_COLOR = 0xFFE070C0;
    private static final int WARNING_COLOR = 0xFFFF8060;

    private ContamLoadHud() {
    }

    public static List<HudRenderCommand> buildCommands(
        FalseSkinHudStateStore.State state,
        int screenWidth,
        int screenHeight
    ) {
        FalseSkinHudStateStore.State safeState = state == null ? FalseSkinHudStateStore.State.NONE : state;
        if (!safeState.active() || screenWidth <= 0 || screenHeight <= 0) return List.of();

        int x = MiniBodyHudPlanner.MARGIN_X + MiniBodyHudPlanner.PANEL_W + 4;
        int y = screenHeight - MiniBodyHudPlanner.PANEL_H - MiniBodyHudPlanner.MARGIN_Y
            + FalseSkinStackHud.PANEL_H + 3;
        float ratio = safeState.contamRatio();
        int fillW = Math.round((PANEL_W - 6) * ratio);
        int color = ratio >= 0.75f ? WARNING_COLOR : LOAD_COLOR;

        List<HudRenderCommand> out = new ArrayList<>();
        out.add(HudRenderCommand.rect(HudRenderLayer.DERIVED_ATTR, x, y, PANEL_W, PANEL_H, BG_COLOR));
        out.add(HudRenderCommand.text(
            HudRenderLayer.DERIVED_ATTR,
            String.format(Locale.ROOT, "污 %d%%", Math.round(ratio * 100f)),
            x + 3,
            y + 2,
            color
        ));
        out.add(HudRenderCommand.rect(HudRenderLayer.DERIVED_ATTR, x + 3, y + PANEL_H - 4, PANEL_W - 6, 2, TRACK_COLOR));
        if (fillW > 0) {
            out.add(HudRenderCommand.rect(HudRenderLayer.DERIVED_ATTR, x + 3, y + PANEL_H - 4, fillW, 2, color));
        }
        return List.copyOf(out);
    }
}
