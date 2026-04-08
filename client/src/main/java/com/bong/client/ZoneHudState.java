package com.bong.client;

import java.util.List;

public final class ZoneHudState {
    private static final long ENTRY_BANNER_DURATION_MS = 2_000L;

    private static ZoneSnapshot snapshot;

    private ZoneHudState() {
    }

    public static synchronized void update(String zone, double spiritQi, int dangerLevel, List<String> activeEvents) {
        update(zone, spiritQi, dangerLevel, activeEvents, System.currentTimeMillis());
    }

    static synchronized void update(
        String zone,
        double spiritQi,
        int dangerLevel,
        List<String> activeEvents,
        long nowMs
    ) {
        if (zone == null || zone.isBlank() || !Double.isFinite(spiritQi) || dangerLevel < 0 || dangerLevel > 5) {
            return;
        }

        List<String> normalizedActiveEvents = activeEvents == null
            ? List.of()
            : activeEvents.stream().filter(event -> event != null && !event.isBlank()).toList();

        long entryBannerExpiresAtMs = snapshot != null && snapshot.zone().equals(zone)
            ? snapshot.entryBannerExpiresAtMs()
            : nowMs + ENTRY_BANNER_DURATION_MS;

        snapshot = new ZoneSnapshot(zone, spiritQi, dangerLevel, normalizedActiveEvents, entryBannerExpiresAtMs);
    }

    public static synchronized ZoneSnapshot peek() {
        return snapshot;
    }

    public static synchronized boolean shouldShowEntryBanner(long nowMs) {
        return snapshot != null && nowMs < snapshot.entryBannerExpiresAtMs();
    }

    static synchronized void clear() {
        snapshot = null;
    }

    public record ZoneSnapshot(
        String zone,
        double spiritQi,
        int dangerLevel,
        List<String> activeEvents,
        long entryBannerExpiresAtMs
    ) {
    }
}
