package com.bong.client.state;

import java.util.Objects;

public final class RealmCollapseHudState {
    private static final long MILLIS_PER_TICK = 50L;

    private final String zone;
    private final String message;
    private final long startedAtMillis;
    private final int durationTicks;

    private RealmCollapseHudState(String zone, String message, long startedAtMillis, int durationTicks) {
        this.zone = Objects.requireNonNull(zone, "zone");
        this.message = Objects.requireNonNull(message, "message");
        this.startedAtMillis = Math.max(0L, startedAtMillis);
        this.durationTicks = Math.max(0, durationTicks);
    }

    public static RealmCollapseHudState empty() {
        return new RealmCollapseHudState("", "", 0L, 0);
    }

    public static RealmCollapseHudState create(String zone, String message, long startedAtMillis, int durationTicks) {
        String normalizedZone = normalize(zone);
        String normalizedMessage = normalize(message);
        if (normalizedZone.isEmpty() || durationTicks <= 0) {
            return empty();
        }
        return new RealmCollapseHudState(normalizedZone, normalizedMessage, startedAtMillis, durationTicks);
    }

    private static String normalize(String value) {
        return value == null ? "" : value.trim();
    }

    public String zone() {
        return zone;
    }

    public String message() {
        return message;
    }

    public long startedAtMillis() {
        return startedAtMillis;
    }

    public int durationTicks() {
        return durationTicks;
    }

    public int remainingTicks(long nowMillis) {
        if (isEmpty()) {
            return 0;
        }
        long elapsedTicks = Math.max(0L, nowMillis - startedAtMillis) / MILLIS_PER_TICK;
        return (int) Math.max(0L, durationTicks - elapsedTicks);
    }

    public boolean active(long nowMillis) {
        return remainingTicks(nowMillis) > 0;
    }

    public boolean isEmpty() {
        return zone.isEmpty() || durationTicks == 0;
    }
}
