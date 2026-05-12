package com.bong.client.death;

import com.bong.client.hud.HudRenderCommand;
import com.bong.client.hud.HudRenderLayer;

import java.util.ArrayList;
import java.util.List;

public final class DeathCinematicRenderer {
    private DeathCinematicRenderer() {}

    public static List<HudRenderCommand> buildCommands(
        DeathCinematicState state,
        long nowMillis,
        int width,
        int height
    ) {
        if (state == null || !state.active() || width <= 0 || height <= 0) return List.of();
        DeathCinematicState current = state.advancedTo(nowMillis);
        if (current.finalDeath() && current.phase() == DeathCinematicState.Phase.INSIGHT_OVERLAY) {
            return FinalWordsRenderer.buildCommands(current, width, height);
        }

        return switch (current.phase()) {
            case PREDEATH -> NearDeathCollapsePlanner.buildCommands(current, width, height);
            case DEATH_MOMENT -> ScreenShatterEffect.buildCommands(current, width, height);
            case ROLL -> DeathRollUI.buildCommands(current, width, height);
            case INSIGHT_OVERLAY -> InsightOverlayRenderer.buildCommands(current, width, height);
            case DARKNESS -> darknessCommands(width, height, nowMillis);
            case REBIRTH -> RebirthCinematicRenderer.buildCommands(current, width, height);
        };
    }

    private static List<HudRenderCommand> darknessCommands(int width, int height, long nowMillis) {
        List<HudRenderCommand> out = new ArrayList<>();
        out.add(HudRenderCommand.screenTint(HudRenderLayer.VISUAL, 0xF8000000));
        if ((nowMillis / 500L) % 2L == 0L) {
            out.add(HudRenderCommand.text(HudRenderLayer.VISUAL, "万籁俱寂", width / 2 - 30, height / 2, 0xFF706A60));
        }
        return out;
    }
}
