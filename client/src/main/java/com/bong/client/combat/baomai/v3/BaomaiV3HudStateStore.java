package com.bong.client.combat.baomai.v3;

public final class BaomaiV3HudStateStore {
    private static final long TICK_MS = 50L;
    private static final long SCAR_VISIBLE_MS = 6_000L;

    private static long bloodBurnUntilMs;
    private static long bloodBurnStartedMs;
    private static long transcendUntilMs;
    private static long transcendStartedMs;
    private static double flowRateMultiplier;
    private static long scarUpdatedMs;
    private static double scarSeverity;

    private BaomaiV3HudStateStore() {}

    public static void clear() {
        bloodBurnUntilMs = 0L;
        bloodBurnStartedMs = 0L;
        transcendUntilMs = 0L;
        transcendStartedMs = 0L;
        flowRateMultiplier = 0.0;
        scarUpdatedMs = 0L;
        scarSeverity = 0.0;
    }

    public static void recordBloodBurn(int durationTicks) {
        recordBloodBurn(durationTicks, System.currentTimeMillis());
    }

    static void recordBloodBurn(int durationTicks, long nowMs) {
        bloodBurnStartedMs = nowMs;
        bloodBurnUntilMs = nowMs + Math.max(1L, durationTicks) * TICK_MS;
    }

    public static void recordBodyTranscendence(int durationTicks, double flowMultiplier) {
        recordBodyTranscendence(durationTicks, flowMultiplier, System.currentTimeMillis());
    }

    static void recordBodyTranscendence(int durationTicks, double flowMultiplier, long nowMs) {
        transcendStartedMs = nowMs;
        transcendUntilMs = nowMs + Math.max(1L, durationTicks) * TICK_MS;
        flowRateMultiplier = Math.max(1.0, flowMultiplier);
    }

    public static void recordMeridianRippleScar(double severity) {
        recordMeridianRippleScar(severity, System.currentTimeMillis());
    }

    static void recordMeridianRippleScar(double severity, long nowMs) {
        scarUpdatedMs = nowMs;
        scarSeverity = Math.max(scarSeverity, clamp01(severity));
    }

    public static Snapshot snapshot(long nowMs) {
        boolean bloodActive = bloodBurnUntilMs > nowMs;
        boolean transcendActive = transcendUntilMs > nowMs;
        boolean scarVisible = scarUpdatedMs > 0L && nowMs - scarUpdatedMs <= SCAR_VISIBLE_MS;
        return new Snapshot(
            bloodActive,
            bloodActive ? progress(nowMs, bloodBurnStartedMs, bloodBurnUntilMs) : 0.0,
            bloodActive ? remainingTicks(nowMs, bloodBurnUntilMs) : 0L,
            transcendActive,
            transcendActive ? progress(nowMs, transcendStartedMs, transcendUntilMs) : 0.0,
            transcendActive ? remainingTicks(nowMs, transcendUntilMs) : 0L,
            transcendActive ? flowRateMultiplier : 0.0,
            scarVisible,
            scarVisible ? scarSeverity : 0.0
        );
    }

    private static double progress(long nowMs, long startedMs, long untilMs) {
        long total = Math.max(1L, untilMs - startedMs);
        long remaining = Math.max(0L, untilMs - nowMs);
        return clamp01((double) remaining / (double) total);
    }

    private static long remainingTicks(long nowMs, long untilMs) {
        return Math.max(0L, (long) Math.ceil((untilMs - nowMs) / (double) TICK_MS));
    }

    private static double clamp01(double value) {
        if (!Double.isFinite(value)) {
            return 0.0;
        }
        return Math.max(0.0, Math.min(1.0, value));
    }

    public record Snapshot(
        boolean bloodBurnActive,
        double bloodBurnProgress,
        long bloodBurnRemainingTicks,
        boolean bodyTranscendenceActive,
        double bodyTranscendenceProgress,
        long bodyTranscendenceRemainingTicks,
        double flowRateMultiplier,
        boolean meridianRippleScarVisible,
        double meridianRippleScarSeverity
    ) {}
}
