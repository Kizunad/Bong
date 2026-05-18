package com.bong.client.dandao;

import com.bong.client.hud.HudRenderCommand;
import com.bong.client.hud.HudRenderLayer;

import java.util.ArrayList;
import java.util.List;

/**
 * plan-dandao-path-v1 P3 -- HUD planner for dandao mutation status display.
 *
 * <p>Only renders when {@link MutationVisualState#stage()} >= 1
 * (conditional display per {@code feedback_hud_conditional.md}).
 *
 * <p>Shows: stage indicator + cumulative toxin progress bar + meridian penalty
 * + active mutation slot list. ExtraHand slots only appear when multi-arm
 * mutation is unlocked (§8.1 #2 HUD conditional display constraint).
 */
public final class MutationHudPlanner {
    static final int BAR_WIDTH = 60;
    static final int BAR_HEIGHT = 4;
    static final double[] THRESHOLDS = {30.0, 100.0, 250.0, 500.0};
    private static final int SLOT_LINE_HEIGHT = 10;

    private MutationHudPlanner() {}

    public static List<HudRenderCommand> buildCommands(int screenWidth, int screenHeight) {
        int stage = MutationVisualState.stage();
        if (stage <= 0) return List.of();

        List<HudRenderCommand> out = new ArrayList<>();
        int x = screenWidth - BAR_WIDTH - 8;
        int y = screenHeight - 60;

        // Stage indicator text
        String stageLabel = stageLabel(stage);
        out.add(HudRenderCommand.text(
            HudRenderLayer.DANDAO_MUTATION,
            "丹体：" + stageLabel,
            x, y - 10,
            0xFFCCCCCC
        ));

        // Progress bar to next stage
        double toxin = MutationVisualState.cumulativeToxin();
        double progress = computeProgress(stage, toxin);
        int filled = (int)(BAR_WIDTH * progress);

        int barColor = barColor(stage);
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

        // Active mutation slot list
        List<MutationVisualState.MutationSlotEntry> slots = MutationVisualState.activeSlots();
        if (!slots.isEmpty()) {
            int slotY = y + BAR_HEIGHT + (penalty > 0.0 ? 14 : 6);
            for (MutationVisualState.MutationSlotEntry slot : slots) {
                String kindDisplay = MutationInspectLabel.translateKind(slot.kind());
                int slotColor = slotColor(slot.level());
                out.add(HudRenderCommand.text(
                    HudRenderLayer.DANDAO_MUTATION,
                    "◆ " + kindDisplay,
                    x, slotY,
                    slotColor
                ));
                slotY += SLOT_LINE_HEIGHT;
            }
        }

        return out;
    }

    static String stageLabel(int stage) {
        return switch (stage) {
            case 1 -> "§a微变";
            case 2 -> "§e显变";
            case 3 -> "§6重变";
            case 4 -> "§c兽化";
            default -> "§7--";
        };
    }

    static double computeProgress(int stage, double toxin) {
        int clampedStage = Math.min(stage, 4);
        double nextThreshold = clampedStage < 4 ? THRESHOLDS[clampedStage] : THRESHOLDS[3];
        double prevThreshold = clampedStage >= 2 ? THRESHOLDS[clampedStage - 2] : 0.0;
        double range = nextThreshold - prevThreshold;
        return range > 0.0 ? Math.min(1.0, Math.max(0.0, (toxin - prevThreshold) / range)) : 1.0;
    }

    static int barColor(int stage) {
        return switch (stage) {
            case 1 -> 0xAA7ED4A0;
            case 2 -> 0xAAD4C87E;
            case 3 -> 0xAAD4A07E;
            case 4 -> 0xAAD47E7E;
            default -> 0xAA888888;
        };
    }

    private static int slotColor(int level) {
        return switch (level) {
            case 1 -> 0xFF88AA88;
            case 2 -> 0xFFAACC88;
            case 3 -> 0xFFDDAA66;
            default -> 0xFF999999;
        };
    }

    /**
     * Whether multi-arm mutation is active (for ExtraHand HUD condition).
     * §8.1 #2: ExtraHand slots only show when multi-arm is unlocked.
     */
    public static boolean hasExtraArms() {
        return MutationVisualState.activeSlots().stream()
            .anyMatch(slot -> "ExtraArms".equals(slot.kind()));
    }
}
