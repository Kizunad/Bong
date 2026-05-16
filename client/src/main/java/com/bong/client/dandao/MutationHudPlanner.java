package com.bong.client.dandao;

import com.bong.client.hud.HudRenderCommand;
import com.bong.client.hud.HudRenderLayer;

import java.util.ArrayList;
import java.util.List;

/**
 * plan-dandao-path-v1 P3 — HUD planner for mutation status display.
 *
 * Only renders when mutation stage >= 1.
 * Shows: stage indicator + cumulative toxin progress bar + meridian penalty.
 */
public final class MutationHudPlanner {
    private static final int BAR_WIDTH = 60;
    private static final int BAR_HEIGHT = 4;
    private static final double[] THRESHOLDS = {30.0, 100.0, 250.0, 500.0};

    private MutationHudPlanner() {}

    public static List<HudRenderCommand> buildCommands(int screenWidth, int screenHeight) {
        int stage = MutationVisualState.stage();
        if (stage <= 0) return List.of();

        List<HudRenderCommand> out = new ArrayList<>();
        int x = screenWidth - BAR_WIDTH - 8;
        int y = screenHeight - 60;

        // Stage indicator text
        String stageLabel = switch (stage) {
            case 1 -> "§a微变";
            case 2 -> "§e显变";
            case 3 -> "§6重变";
            case 4 -> "§c兽化";
            default -> "§7--";
        };
        out.add(HudRenderCommand.text(
            HudRenderLayer.DANDAO_MUTATION,
            "丹体：" + stageLabel,
            x, y - 10,
            0xFFCCCCCC
        ));

        // Progress bar to next stage
        double toxin = MutationVisualState.cumulativeToxin();
        double nextThreshold = stage < 4 ? THRESHOLDS[stage] : THRESHOLDS[3];
        double prevThreshold = stage > 1 ? THRESHOLDS[stage - 2] : 0.0;
        double progress = Math.min(1.0, (toxin - prevThreshold) / (nextThreshold - prevThreshold));
        int filled = (int)(BAR_WIDTH * progress);

        int barColor = switch (stage) {
            case 1 -> 0xAA7ED4A0;
            case 2 -> 0xAAD4C87E;
            case 3 -> 0xAAD4A07E;
            case 4 -> 0xAAD47E7E;
            default -> 0xAA888888;
        };

        out.add(HudRenderCommand.rect(HudRenderLayer.DANDAO_MUTATION, x, y, BAR_WIDTH, BAR_HEIGHT, 0x44202020));
        out.add(HudRenderCommand.rect(HudRenderLayer.DANDAO_MUTATION, x, y, filled, BAR_HEIGHT, barColor));

        // Meridian penalty indicator
        double penalty = MutationVisualState.meridianPenalty();
        if (penalty > 0.0) {
            String penaltyText = String.format("§7经脉 -%.0f%%", penalty * 100);
            out.add(HudRenderCommand.text(
                HudRenderLayer.DANDAO_MUTATION,
                penaltyText,
                x, y + BAR_HEIGHT + 2,
                0xFF999999
            ));
        }

        return out;
    }
}
