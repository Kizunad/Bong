package com.bong.client.social;

import com.bong.client.combat.UnifiedEvent;
import com.bong.client.combat.UnifiedEventStore;

import java.util.List;

public final class NicheIntrusionAlertHandler {
    private static final int WARNING_COLOR = 0xFFFFAA55;
    private static final int SOCIAL_COLOR = 0xFFA0C0FF;

    private NicheIntrusionAlertHandler() {
    }

    public static void recordIntrusion(String intruderId, List<Long> itemsTaken, double taintDelta) {
        NicheGuardianStore.recordIntrusion(new NicheGuardianStore.NicheIntrusionAlert(
            itemsTaken,
            intruderId,
            taintDelta,
            System.currentTimeMillis()
        ));
        UnifiedEventStore.stream().publish(
            UnifiedEvent.Channel.SOCIAL,
            UnifiedEvent.Priority.P1_IMPORTANT,
            "niche_intrusion:" + fallback(intruderId),
            "灵龛入侵：" + fallback(intruderId) + " 取走 " + (itemsTaken == null ? 0 : itemsTaken.size()) + " 件",
            WARNING_COLOR,
            System.currentTimeMillis()
        );
    }

    public static void recordGuardianFatigue(String guardianKind, int chargesRemaining) {
        NicheGuardianStore.recordFatigue(guardianKind, chargesRemaining);
        UnifiedEventStore.stream().publish(
            UnifiedEvent.Channel.SOCIAL,
            UnifiedEvent.Priority.P2_NORMAL,
            "niche_guardian_fatigue:" + fallback(guardianKind),
            "守家载体损耗：" + fallback(guardianKind) + " 剩余 " + Math.max(0, chargesRemaining) + " 次",
            SOCIAL_COLOR,
            System.currentTimeMillis()
        );
    }

    public static void recordGuardianBroken(String guardianKind, String intruderId) {
        NicheGuardianStore.recordBroken(guardianKind, intruderId);
        UnifiedEventStore.stream().publish(
            UnifiedEvent.Channel.SOCIAL,
            UnifiedEvent.Priority.P1_IMPORTANT,
            "niche_guardian_broken:" + fallback(guardianKind),
            "守家载体破损：" + fallback(guardianKind),
            WARNING_COLOR,
            System.currentTimeMillis()
        );
    }

    private static String fallback(String value) {
        return value == null || value.isBlank() ? "unknown" : value.trim();
    }
}
