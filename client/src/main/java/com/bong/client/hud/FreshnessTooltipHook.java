package com.bong.client.hud;

import com.bong.client.processing.state.FreshnessStore;

import java.util.Locale;

/** plan-lingtian-process-v1 P3 — inventory tooltip freshness 文案。 */
public final class FreshnessTooltipHook {
    private FreshnessTooltipHook() {}

    public static String tooltipLine(String itemUuid) {
        FreshnessStore.Entry entry = FreshnessStore.get(itemUuid);
        if (entry == null) {
            return "";
        }
        return formatTooltipLine(entry.freshness(), entry.profileName());
    }

    public static String formatTooltipLine(float freshness, String profileName) {
        int pct = Math.round(Math.max(0.0f, Math.min(1.0f, freshness)) * 100.0f);
        String profile = profileName == null ? "" : profileName.toLowerCase(Locale.ROOT);
        String profileLabel = profile.contains("linear") || profile.contains("fresh") || profile.contains("forging")
            ? "Linear"
            : "Exponential";
        return "鲜度: " + pct + "/100 · " + profileLabel;
    }
}
