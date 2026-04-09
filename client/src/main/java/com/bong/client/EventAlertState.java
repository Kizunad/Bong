package com.bong.client;

import java.util.Objects;

public final class EventAlertState {
    static final long BANNER_DURATION_MS = 6_000L;
    static final long FADE_IN_DURATION_MS = 250L;
    static final long FADE_OUT_DURATION_MS = 500L;
    static final int MAX_TITLE_LENGTH = 24;
    static final int MAX_DETAIL_LENGTH = 72;
    static final int MAX_ZONE_LENGTH = 24;

    private static AlertSnapshot latestAlert;

    private EventAlertState() {
    }

    static AlertSnapshot recordAlert(BongServerPayload.EventAlert eventAlert) {
        return recordAlert(eventAlert, System.currentTimeMillis());
    }

    static AlertSnapshot recordAlert(BongServerPayload.EventAlert eventAlert, long nowMs) {
        Objects.requireNonNull(eventAlert, "eventAlert");

        AlertSnapshot snapshot = snapshotOf(eventAlert, nowMs);
        latestAlert = snapshot;
        return snapshot;
    }

    static AlertSnapshot snapshotOf(BongServerPayload.EventAlert eventAlert, long nowMs) {
        Objects.requireNonNull(eventAlert, "eventAlert");

        return new AlertSnapshot(
                clipText(eventAlert.title(), MAX_TITLE_LENGTH),
                clipText(eventAlert.detail(), MAX_DETAIL_LENGTH),
                normalizeZoneLabel(eventAlert.zone()),
                Severity.fromWire(eventAlert.severity()),
                nowMs,
                nowMs + BANNER_DURATION_MS
        );
    }

    private static String normalizeZoneLabel(String zone) {
        if (zone == null) {
            return null;
        }

        return ZoneState.clipLabel(ZoneState.humanizeZoneName(zone), MAX_ZONE_LENGTH);
    }

    static String clipText(String text, int maxChars) {
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

    static int bannerAlpha(long nowMs, long recordedAtMs, long expiresAtMs) {
        if (nowMs >= expiresAtMs) {
            return 0;
        }

        if (nowMs <= recordedAtMs) {
            return 0;
        }

        long fadeInEnd = recordedAtMs + FADE_IN_DURATION_MS;
        if (nowMs < fadeInEnd) {
            return scaleAlpha(nowMs - recordedAtMs, FADE_IN_DURATION_MS);
        }

        long fadeOutStart = expiresAtMs - FADE_OUT_DURATION_MS;
        if (nowMs >= fadeOutStart) {
            return scaleAlpha(expiresAtMs - nowMs, FADE_OUT_DURATION_MS);
        }

        return 255;
    }

    private static int scaleAlpha(long elapsedMs, long durationMs) {
        if (durationMs <= 0L) {
            return 255;
        }

        double clampedRatio = Math.max(0.0d, Math.min(1.0d, (double) elapsedMs / (double) durationMs));
        return clampAlpha((int) Math.round(255.0d * clampedRatio));
    }

    static int clampAlpha(int alpha) {
        return Math.max(0, Math.min(255, alpha));
    }

    static AlertSnapshot getLatestAlert() {
        return latestAlert;
    }

    public static BannerState getCurrentBanner() {
        return getCurrentBanner(System.currentTimeMillis());
    }

    static BannerState getCurrentBanner(long nowMs) {
        AlertSnapshot snapshot = latestAlert;
        if (snapshot == null || nowMs >= snapshot.expiresAtMs()) {
            return null;
        }

        return new BannerState(
                snapshot.title(),
                snapshot.detail(),
                snapshot.zoneLabel(),
                snapshot.severity(),
                bannerAlpha(nowMs, snapshot.recordedAtMs(), snapshot.expiresAtMs()),
                snapshot.expiresAtMs()
        );
    }

    public static void clear() {
        latestAlert = null;
    }

    record AlertSnapshot(String title, String detail, String zoneLabel, Severity severity, long recordedAtMs,
                         long expiresAtMs) {
        AlertSnapshot {
            Objects.requireNonNull(title, "title");
            Objects.requireNonNull(detail, "detail");
            Objects.requireNonNull(severity, "severity");
        }
    }

    public record BannerState(String title, String detail, String zoneLabel, Severity severity, int alpha,
                              long expiresAtMs) {
        public BannerState {
            Objects.requireNonNull(title, "title");
            Objects.requireNonNull(detail, "detail");
            Objects.requireNonNull(severity, "severity");
        }
    }

    enum Severity {
        INFO("info", "INFO", 0x7FDBFF),
        WARNING("warning", "WARNING", 0xFFB347),
        CRITICAL("critical", "CRITICAL", 0xFF5555);

        private final String wireName;
        private final String label;
        private final int accentColor;

        Severity(String wireName, String label, int accentColor) {
            this.wireName = wireName;
            this.label = label;
            this.accentColor = accentColor;
        }

        static Severity fromWire(String severity) {
            for (Severity candidate : values()) {
                if (candidate.wireName.equals(severity)) {
                    return candidate;
                }
            }

            return INFO;
        }

        String label() {
            return label;
        }

        int accentColor() {
            return accentColor;
        }
    }
}
