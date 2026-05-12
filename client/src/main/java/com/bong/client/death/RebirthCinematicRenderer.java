package com.bong.client.death;

import com.bong.client.hud.HudRenderCommand;
import com.bong.client.hud.HudRenderLayer;

import java.util.ArrayList;
import java.util.List;

public final class RebirthCinematicRenderer {
    private RebirthCinematicRenderer() {}

    public static List<HudRenderCommand> buildCommands(DeathCinematicState state, int width, int height) {
        if (state == null || !state.active() || width <= 0 || height <= 0) return List.of();
        double progress = state.phaseProgress();
        int darkness = (int) Math.round((1.0 - progress) * 220);
        List<HudRenderCommand> out = new ArrayList<>();
        out.add(HudRenderCommand.screenTint(HudRenderLayer.VISUAL, (darkness << 24)));
        out.add(HudRenderCommand.edgeVignette(HudRenderLayer.VISUAL, 0x446E6A76));
        out.add(HudRenderCommand.text(
            HudRenderLayer.VISUAL,
            state.tsyDeath() ? "坍缩渊雾散，灵龛在外" : "灵龛微光重新照见你",
            width / 2 - 72,
            height / 2 + 28,
            0xFFD0CCD8
        ));
        out.add(HudRenderCommand.text(
            HudRenderLayer.VISUAL,
            "虚弱 " + Math.max(1L, state.rebirthWeakenedTicks() / 20L) + "s",
            width / 2 - 32,
            height / 2 + 44,
            0xFFC0B090
        ));
        return out;
    }
}
