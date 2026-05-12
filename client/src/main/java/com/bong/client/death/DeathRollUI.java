package com.bong.client.death;

import com.bong.client.hud.HudRenderCommand;
import com.bong.client.hud.HudRenderLayer;

import java.util.ArrayList;
import java.util.List;

public final class DeathRollUI {
    private static final double ROLL_ANIMATION_PHASE_RATIO = 0.38;

    private DeathRollUI() {}

    public static double displayedProbability(DeathCinematicState state) {
        if (state == null || !state.active()) return 0.0;
        double progress = Math.min(1.0, state.phaseProgress() / ROLL_ANIMATION_PHASE_RATIO);
        double target = state.roll().probability();
        return 1.0 - (1.0 - target) * progress;
    }

    public static List<String> bambooSlipLabels(DeathCinematicState.RollResult result) {
        return switch (result == null ? DeathCinematicState.RollResult.PENDING : result) {
            case SURVIVE -> List.of("生", "生", "生");
            case FALL -> List.of("落", "落", "生");
            case FINAL -> List.of("终", "终", "碎");
            case PENDING -> List.of("?", "?", "?");
        };
    }

    public static List<HudRenderCommand> buildCommands(DeathCinematicState state, int width, int height) {
        if (state == null || !state.active() || width <= 0 || height <= 0) return List.of();
        List<HudRenderCommand> out = new ArrayList<>();
        out.add(HudRenderCommand.screenTint(HudRenderLayer.VISUAL, 0xE8000000));
        String label = state.roll().result() == DeathCinematicState.RollResult.FINAL ? "终焉" :
            state.roll().result() == DeathCinematicState.RollResult.PENDING ? "劫数" : "运数";
        out.add(HudRenderCommand.scaledText(
            HudRenderLayer.VISUAL,
            label,
            width / 2 - 24,
            height / 2 - 62,
            0xFFC0B090,
            1.4
        ));
        out.add(HudRenderCommand.scaledText(
            HudRenderLayer.VISUAL,
            Math.round(displayedProbability(state) * 100) + "%",
            width / 2 - 28,
            height / 2 - 30,
            0xFFE0C040,
            1.8
        ));
        List<String> labels = bambooSlipLabels(state.roll().result());
        for (int i = 0; i < labels.size(); i++) {
            out.add(HudRenderCommand.text(
                HudRenderLayer.VISUAL,
                labels.get(i),
                width / 2 - 30 + i * 28,
                height / 2 + 20 + (int) (Math.sin(state.phaseProgress() * Math.PI + i) * 4),
                slipColor(state.roll().result())
            ));
        }
        return out;
    }

    private static int slipColor(DeathCinematicState.RollResult result) {
        return switch (result == null ? DeathCinematicState.RollResult.PENDING : result) {
            case SURVIVE -> 0xFF80D090;
            case FALL -> 0xFFE06060;
            case FINAL -> 0xFFFFD060;
            case PENDING -> 0xFFC0B090;
        };
    }
}
