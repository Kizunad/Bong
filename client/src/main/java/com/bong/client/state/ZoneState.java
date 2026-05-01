package com.bong.client.state;

import java.util.Objects;
import java.util.Set;

public final class ZoneState {
    private static final int MIN_DANGER_LEVEL = 0;
    private static final int MAX_DANGER_LEVEL = 5;

    private final String zoneId;
    private final String zoneLabel;
    private final double spiritQiNormalized;
    private final int dangerLevel;
    private final boolean noCadence;
    private final long changedAtMillis;

    private ZoneState(String zoneId, String zoneLabel, double spiritQiNormalized, int dangerLevel, boolean noCadence, long changedAtMillis) {
        this.zoneId = Objects.requireNonNull(zoneId, "zoneId");
        this.zoneLabel = Objects.requireNonNull(zoneLabel, "zoneLabel");
        this.spiritQiNormalized = spiritQiNormalized;
        this.dangerLevel = dangerLevel;
        this.noCadence = noCadence;
        this.changedAtMillis = changedAtMillis;
    }

    public static ZoneState empty() {
        return new ZoneState("", "", 0.0, 0, false, 0L);
    }

    public static ZoneState create(String zoneId, String zoneLabel, double spiritQiNormalized, int dangerLevel, long changedAtMillis) {
        return create(zoneId, zoneLabel, spiritQiNormalized, dangerLevel, Set.of(), changedAtMillis);
    }

    public static ZoneState create(
        String zoneId,
        String zoneLabel,
        double spiritQiNormalized,
        int dangerLevel,
        Set<String> activeEvents,
        long changedAtMillis
    ) {
        String normalizedZoneId = normalizeText(zoneId);
        if (normalizedZoneId.isEmpty()) {
            return empty();
        }

        String normalizedZoneLabel = normalizeText(zoneLabel);
        if (normalizedZoneLabel.isEmpty()) {
            normalizedZoneLabel = normalizedZoneId;
        }

        return new ZoneState(
            normalizedZoneId,
            normalizedZoneLabel,
            clamp(spiritQiNormalized, 0.0, 1.0),
            clamp(dangerLevel, MIN_DANGER_LEVEL, MAX_DANGER_LEVEL),
            containsNoCadence(activeEvents),
            Math.max(0L, changedAtMillis)
        );
    }

    private static boolean containsNoCadence(Set<String> activeEvents) {
        if (activeEvents == null || activeEvents.isEmpty()) {
            return false;
        }
        return activeEvents.stream()
            .filter(Objects::nonNull)
            .map(String::trim)
            .anyMatch("no_cadence"::equalsIgnoreCase);
    }

    private static String normalizeText(String value) {
        return value == null ? "" : value.trim();
    }

    private static double clamp(double value, double min, double max) {
        if (!Double.isFinite(value)) {
            return min;
        }
        return Math.max(min, Math.min(max, value));
    }

    private static int clamp(int value, int min, int max) {
        return Math.max(min, Math.min(max, value));
    }

    public String zoneId() {
        return zoneId;
    }

    public String zoneLabel() {
        return zoneLabel;
    }

    public double spiritQiNormalized() {
        return spiritQiNormalized;
    }

    public int dangerLevel() {
        return dangerLevel;
    }

    public boolean noCadence() {
        return noCadence;
    }

    public long changedAtMillis() {
        return changedAtMillis;
    }

    public boolean isEmpty() {
        return zoneId.isEmpty();
    }
}
