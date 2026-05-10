package com.bong.client.hud;

import java.util.Locale;

final class HudRealmGate {
    private HudRealmGate() {
    }

    static boolean atLeast(String realm, int requiredTier) {
        return tier(realm) >= requiredTier;
    }

    static boolean atLeastCondense(String realm) {
        return atLeast(realm, 2);
    }

    static boolean atLeastSpirit(String realm) {
        return atLeast(realm, 4);
    }

    static boolean atLeastVoid(String realm) {
        return atLeast(realm, 5);
    }

    static int tier(String realm) {
        if (realm == null) {
            return 0;
        }
        String normalized = realm.trim().toLowerCase(Locale.ROOT);
        return switch (normalized) {
            case "醒灵", "awaken" -> 0;
            case "引气", "induce" -> 1;
            case "凝脉", "condense" -> 2;
            case "固元", "solidify" -> 3;
            case "通灵", "化神", "spirit" -> 4;
            case "化虚", "void" -> 5;
            default -> 0;
        };
    }
}
