package com.bong.client.hud;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

public final class PillBuffHudPlanner {

    private static final int TYPICAL_MAX_TICKS = 6000;

    public record PillBuff(String buffId, int remainingTicks, double effectMultiplier) {
        public boolean isExpired() {
            return remainingTicks <= 0;
        }

        public float progress() {
            if (remainingTicks <= 0) return 0.0f;
            return Math.min(1.0f, (float) remainingTicks / TYPICAL_MAX_TICKS);
        }
    }

    private static final List<PillBuff> activeBuffs = new ArrayList<>();

    public static void updateBuff(String buffId, int remainingTicks, double effectMultiplier) {
        activeBuffs.removeIf(b -> b.buffId().equals(buffId));
        if (remainingTicks > 0) {
            activeBuffs.add(new PillBuff(buffId, remainingTicks, effectMultiplier));
        }
    }

    public static List<PillBuff> activeBuffs() {
        activeBuffs.removeIf(PillBuff::isExpired);
        return Collections.unmodifiableList(new ArrayList<>(activeBuffs));
    }

    public static void clear() {
        activeBuffs.clear();
    }

    private PillBuffHudPlanner() {
    }
}
