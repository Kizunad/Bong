package com.bong.client;

import java.util.Locale;
import java.util.Objects;

public final class ZoneState {
    static final double MIN_SPIRIT_QI = 0.0d;
    static final double MAX_SPIRIT_QI = 1.0d;
    static final int MAX_DANGER_LEVEL = 5;
    static final int MAX_ZONE_LABEL_LENGTH = 24;

    private static ZoneHudState currentZone;

    private ZoneState() {
    }

    static ZoneHudState recordZoneInfo(BongServerPayload.ZoneInfo zoneInfo) {
        return recordZoneInfo(zoneInfo, System.currentTimeMillis());
    }

    static ZoneHudState recordZoneInfo(BongServerPayload.ZoneInfo zoneInfo, long nowMs) {
        Objects.requireNonNull(zoneInfo, "zoneInfo");

        ZoneHudState snapshot = snapshotOf(zoneInfo, nowMs);
        currentZone = snapshot;
        return snapshot;
    }

    static ZoneHudState snapshotOf(BongServerPayload.ZoneInfo zoneInfo, long nowMs) {
        Objects.requireNonNull(zoneInfo, "zoneInfo");

        return new ZoneHudState(
                clipLabel(humanizeZoneName(zoneInfo.zone()), MAX_ZONE_LABEL_LENGTH),
                clampSpiritQi(zoneInfo.spiritQi()),
                clampDangerLevel(zoneInfo.dangerLevel()),
                nowMs
        );
    }

    static double clampSpiritQi(double spiritQi) {
        if (!Double.isFinite(spiritQi)) {
            return MIN_SPIRIT_QI;
        }

        return Math.max(MIN_SPIRIT_QI, Math.min(MAX_SPIRIT_QI, spiritQi));
    }

    static int clampDangerLevel(int dangerLevel) {
        return Math.max(0, Math.min(MAX_DANGER_LEVEL, dangerLevel));
    }

    static String humanizeZoneName(String zone) {
        Objects.requireNonNull(zone, "zone");

        String normalized = zone
                .replace('-', ' ')
                .replace('_', ' ')
                .trim()
                .replaceAll("\\s+", " ");

        if (normalized.isEmpty()) {
            return "Unknown Zone";
        }

        String[] tokens = normalized.split(" ");
        StringBuilder builder = new StringBuilder();
        for (String token : tokens) {
            if (builder.length() > 0) {
                builder.append(' ');
            }
            builder.append(titleToken(token));
        }

        return builder.toString();
    }

    static String clipLabel(String text, int maxChars) {
        Objects.requireNonNull(text, "text");
        if (maxChars < 4) {
            throw new IllegalArgumentException("maxChars must be at least 4");
        }

        String normalized = text.trim();
        if (normalized.length() <= maxChars) {
            return normalized;
        }

        return normalized.substring(0, maxChars - 3).trim() + "...";
    }

    private static String titleToken(String token) {
        if (token.isEmpty()) {
            return token;
        }

        if (token.length() == 1) {
            return token.toUpperCase(Locale.ROOT);
        }

        return token.substring(0, 1).toUpperCase(Locale.ROOT) + token.substring(1).toLowerCase(Locale.ROOT);
    }

    public static ZoneHudState getCurrentZone() {
        return currentZone;
    }

    public static void clear() {
        currentZone = null;
    }

    public record ZoneHudState(String zoneLabel, double spiritQi, int dangerLevel, long changedAtMs) {
        public ZoneHudState {
            Objects.requireNonNull(zoneLabel, "zoneLabel");
        }
    }
}
