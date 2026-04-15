package com.bong.client.hud;

import com.bong.client.combat.store.DamageFloaterStore;

import java.util.ArrayList;
import java.util.List;

/**
 * Renders damage floaters as simple rising text near the screen center (plan
 * §U1 / §1 "伤害飘字"). World-space projection is intentionally a best-effort
 * fallback: if no camera info is available, floaters stack along the mid-right
 * with a vertical animation driven by {@code createdAtMs}.
 */
public final class DamageFloaterHudPlanner {
    public static final int LIFETIME_MS = (int) DamageFloaterStore.LIFETIME_MS;
    public static final int RISE_PIXELS = 24;
    public static final int FALLBACK_RIGHT_MARGIN = 120;
    public static final int LINE_STEP = 12;
    public static final int MAX_RENDERED = 10;

    private DamageFloaterHudPlanner() {}

    public static List<HudRenderCommand> buildCommands(
        int screenWidth,
        int screenHeight,
        long nowMs
    ) {
        List<HudRenderCommand> out = new ArrayList<>();
        List<DamageFloaterStore.Floater> floaters = DamageFloaterStore.snapshot(nowMs);
        if (floaters.isEmpty()) return out;

        int baseX = Math.max(24, screenWidth - FALLBACK_RIGHT_MARGIN);
        int baseY = screenHeight / 2;
        int rendered = 0;
        int offset = 0;
        // Iterate from newest back so most recent floaters appear on top.
        for (int i = floaters.size() - 1; i >= 0 && rendered < MAX_RENDERED; i--) {
            DamageFloaterStore.Floater f = floaters.get(i);
            long age = nowMs - f.createdAtMs();
            if (age < 0) age = 0;
            float t = age / (float) LIFETIME_MS;
            if (t > 1f) continue;
            int rise = Math.round(t * RISE_PIXELS);
            int y = baseY - offset - rise;
            String text = f.text();
            if (f.kind() == DamageFloaterStore.Kind.CRIT) text = text + "!";
            if (f.kind() == DamageFloaterStore.Kind.HEAL) text = "+" + text;
            out.add(HudRenderCommand.text(HudRenderLayer.DAMAGE_FLOATER, text, baseX, y, f.color()));
            offset += LINE_STEP;
            rendered++;
        }
        return out;
    }
}
