package com.bong.client.death;

import com.bong.client.hud.HudRenderCommand;
import com.bong.client.hud.HudRenderLayer;

import java.util.ArrayList;
import java.util.List;

public final class FinalWordsRenderer {
    private FinalWordsRenderer() {}

    public static List<HudRenderCommand> buildCommands(DeathCinematicState state, int width, int height) {
        if (state == null || !state.active() || width <= 0 || height <= 0) return List.of();
        List<HudRenderCommand> out = new ArrayList<>();
        out.add(HudRenderCommand.screenTint(HudRenderLayer.VISUAL, 0xF0000000));
        out.add(HudRenderCommand.scaledText(HudRenderLayer.VISUAL, "终焉之言", width / 2 - 52, height / 2 - 50, 0xFFE0C060, 1.6));
        int y = height / 2 - 4;
        for (int i = 0; i < Math.min(6, state.insightText().size()); i++) {
            out.add(HudRenderCommand.text(
                HudRenderLayer.VISUAL,
                state.insightText().get(i),
                width / 2 - 60,
                y + i * 14,
                0xFFC0B090
            ));
        }
        return out;
    }
}
