package com.bong.client.hud;

import com.bong.client.combat.CombatHudState;

import java.util.ArrayList;
import java.util.List;

/**
 * NearDeath post-process (plan §U3 / §2.3). Renders a translucent red screen
 * tint + edge vignette + "hold-on cost" countdown text when hp percent is
 * below {@link #THRESHOLD} and greater than zero.
 */
public final class NearDeathOverlayPlanner {
    public static final float THRESHOLD = 0.12f;
    public static final int TINT_COLOR = 0x40800000;
    public static final int VIGNETTE_COLOR = 0xB0800000;
    public static final int TEXT_COLOR = 0xFFFFB0B0;

    private NearDeathOverlayPlanner() {}

    public static List<HudRenderCommand> buildCommands(
        CombatHudState state,
        int screenWidth,
        int screenHeight
    ) {
        List<HudRenderCommand> out = new ArrayList<>();
        if (state == null || !state.active()) return out;
        float hp = state.hpPercent();
        if (hp <= 0f || hp >= THRESHOLD) return out;
        if (screenWidth <= 0 || screenHeight <= 0) return out;

        out.add(HudRenderCommand.screenTint(HudRenderLayer.NEAR_DEATH, TINT_COLOR));
        out.add(HudRenderCommand.edgeVignette(HudRenderLayer.NEAR_DEATH, VIGNETTE_COLOR));
        String msg = "\u575a\u6301\u4ee3\u4ef7: 0.5 \u771f\u5143/\u79d2";
        out.add(HudRenderCommand.text(
            HudRenderLayer.NEAR_DEATH,
            msg,
            Math.max(8, (screenWidth / 2) - 50),
            screenHeight - 40,
            TEXT_COLOR
        ));
        return out;
    }
}
