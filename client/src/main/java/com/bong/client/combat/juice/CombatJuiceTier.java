package com.bong.client.combat.juice;

import java.util.Locale;

public enum CombatJuiceTier {
    LIGHT,
    HEAVY,
    CRITICAL;

    public static CombatJuiceTier fromWire(String wire) {
        if (wire == null) {
            return null;
        }
        return switch (wire.trim().toLowerCase(Locale.ROOT)) {
            case "light", "minor", "normal" -> LIGHT;
            case "heavy", "full_power", "full_charge", "release" -> HEAVY;
            case "critical", "crit", "overload", "overload_tear" -> CRITICAL;
            default -> null;
        };
    }

    public static CombatJuiceTier fromCombatEvent(String kind, double amount, String explicitTier) {
        CombatJuiceTier parsed = fromWire(explicitTier);
        if (parsed != null) {
            return parsed;
        }
        String normalizedKind = kind == null ? "" : kind.trim().toLowerCase(Locale.ROOT);
        if (normalizedKind.contains("crit") || normalizedKind.contains("overload")) {
            return CRITICAL;
        }
        if (normalizedKind.contains("full_power")
            || normalizedKind.contains("full_charge")
            || normalizedKind.contains("heavy")
            || normalizedKind.contains("release")) {
            return HEAVY;
        }
        if (amount >= 30.0) {
            return CRITICAL;
        }
        if (amount >= 12.0) {
            return HEAVY;
        }
        return LIGHT;
    }
}
