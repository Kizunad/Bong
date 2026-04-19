package com.bong.client.hud;

import com.bong.client.combat.CombatHudState;

import java.util.ArrayList;
import java.util.List;

/**
 * Renders the dedicated stamina bar (plan §U1 / §1 "Stamina 条"). Sits just
 * above the quick-bar row. Hidden when combat HUD is inactive or stamina is
 * full (nothing interesting to show).
 */
public final class StaminaBarHudPlanner {
    public static final int BAR_WIDTH = 120;
    public static final int BAR_HEIGHT = 3;
    public static final int BOTTOM_MARGIN = 62;
    public static final int TRACK_COLOR = 0xC0202830;
    public static final int FILL_COLOR = 0xFFFFD060;
    public static final int LOW_COLOR = 0xFFE06040;

    private StaminaBarHudPlanner() {}

    public static List<HudRenderCommand> buildCommands(
        CombatHudState state,
        int screenWidth,
        int screenHeight
    ) {
        List<HudRenderCommand> out = new ArrayList<>();
        if (state == null || !state.active()) return out;
        float stamina = state.staminaPercent();
        if (stamina >= 0.999f) return out;
        if (screenWidth <= 0 || screenHeight <= 0) return out;

        int x = (screenWidth - BAR_WIDTH) / 2;
        int y = screenHeight - BAR_HEIGHT - BOTTOM_MARGIN;
        out.add(HudRenderCommand.rect(HudRenderLayer.STAMINA_BAR, x, y, BAR_WIDTH, BAR_HEIGHT, TRACK_COLOR));
        int fill = Math.max(0, Math.round(stamina * BAR_WIDTH));
        if (fill > 0) {
            int color = stamina < 0.25f ? LOW_COLOR : FILL_COLOR;
            out.add(HudRenderCommand.rect(HudRenderLayer.STAMINA_BAR, x, y, fill, BAR_HEIGHT, color));
        }
        return out;
    }
}
