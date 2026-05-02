package com.bong.client.util;

import java.util.Locale;
import java.util.Objects;

public final class RealmLabel {
    private RealmLabel() {}

    public static String displayName(String realmKey) {
        Objects.requireNonNull(realmKey, "realmKey");

        String trimmed = realmKey.trim();
        if (trimmed.isEmpty()) {
            return "凡体";
        }

        return switch (trimmed.toLowerCase(Locale.ROOT)) {
            case "awaken" -> "醒灵";
            case "induce" -> "引气";
            case "condense" -> "凝脉";
            case "solidify" -> "固元";
            case "spirit" -> "通灵";
            case "void" -> "化虚";
            default -> trimmed;
        };
    }
}
