package com.bong.client.hud;

/**
 * plan-armor-v1 §5 — 护甲破损 HUD 提示。
 *
 * <p>当 server 推送 ArmorDurabilityChanged{broken=true} 时，
 * 在对应装备部位显示裂纹图标并触发 1s toast。
 */
public final class ArmorBreakIndicator {

    public record BreakEvent(String slot, String templateId, long tickMs) {}

    private static final long TOAST_DURATION_MS = 1000;
    private static volatile BreakEvent lastBreak;

    private ArmorBreakIndicator() {
    }

    public static void onArmorBroken(String slot, String templateId) {
        lastBreak = new BreakEvent(slot, templateId, System.currentTimeMillis());
    }

    public static BreakEvent lastBreak() {
        BreakEvent snapshot = lastBreak;
        if (snapshot == null) return null;
        if (System.currentTimeMillis() - snapshot.tickMs > TOAST_DURATION_MS) {
            lastBreak = null;
            return null;
        }
        return snapshot;
    }

    public static String toastText(BreakEvent event) {
        if (event == null) return null;
        String part = switch (event.slot()) {
            case "head" -> "头盔";
            case "chest" -> "胸甲";
            case "legs" -> "腿甲";
            case "feet" -> "靴";
            default -> event.slot();
        };
        return part + "破损";
    }
}
