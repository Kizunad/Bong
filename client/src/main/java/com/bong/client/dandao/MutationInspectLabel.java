package com.bong.client.dandao;

import java.util.ArrayList;
import java.util.List;
import java.util.Locale;

/**
 * plan-dandao-path-v1 P3 -- Generates inspect labels for viewing another
 * player's mutation state.
 *
 * <p>Rules (§4.3):
 * <ul>
 *   <li>Stage 1: no special label (subtle changes invisible on inspect)</li>
 *   <li>Stage 2+: show "丹体异化·显变" tag + visible mutation list</li>
 *   <li>Stage 4: show "兽化体" red bold label</li>
 * </ul>
 */
public final class MutationInspectLabel {
    private MutationInspectLabel() {}

    /**
     * Builds the inspect label lines for a target's mutation state.
     *
     * @param stage       mutation stage (0-4)
     * @param activeSlots list of active mutation slots
     * @return list of formatted label lines (empty if stage < 2)
     */
    public static List<String> buildLabels(int stage, List<MutationVisualState.MutationSlotEntry> activeSlots) {
        if (stage < 2) {
            return List.of();
        }

        List<String> labels = new ArrayList<>();

        if (stage >= 4) {
            labels.add("§c§l兽化体");
        }

        String stageTag = switch (stage) {
            case 2 -> "§e丹体异化·显变";
            case 3 -> "§6丹体异化·重变";
            case 4 -> "§c丹体异化·兽化";
            default -> "§7丹体异化";
        };
        labels.add(stageTag);

        for (MutationVisualState.MutationSlotEntry slot : activeSlots) {
            String kindDisplay = translateKind(slot.kind());
            String slotDisplay = translateBodySlot(slot.bodySlot());
            labels.add(String.format("§7  ◆ %s Lv.%d [%s]", kindDisplay, slot.level(), slotDisplay));
        }

        return labels;
    }

    /**
     * Whether the given stage is visible on inspect.
     */
    public static boolean isVisibleOnInspect(int stage) {
        return stage >= 2;
    }

    static String translateKind(String kind) {
        if (kind == null) return "未知";
        return switch (kind) {
            case "GoldenIris" -> "金瞳";
            case "HardenedNails" -> "硬甲指";
            case "ToughSkin" -> "糙皮";
            case "BoneRidge" -> "额骨脊";
            case "ForearmScales" -> "前臂鳞";
            case "SpineSpurs" -> "脊突";
            case "Horns" -> "双角";
            case "Tail" -> "尾";
            case "BackCarapace" -> "背甲";
            case "ExtraArms" -> "多臂";
            case "BodyEnlarge" -> "体型膨胀";
            case "BeastFace" -> "兽面";
            default -> kind;
        };
    }

    static String translateBodySlot(String bodySlot) {
        if (bodySlot == null) return "未知";
        return switch (bodySlot.toUpperCase(Locale.ROOT)) {
            case "HEAD" -> "头部";
            case "FOREARM" -> "前臂";
            case "BACK" -> "背部";
            case "TORSO" -> "躯干";
            case "LOWER" -> "下肢";
            default -> bodySlot;
        };
    }
}
