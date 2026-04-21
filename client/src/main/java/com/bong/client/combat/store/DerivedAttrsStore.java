package com.bong.client.combat.store;

/**
 * Authoritative extended derived-attribute state (plan §U6–U7 / §2.4).
 *
 * <p>Flight and phasing contain enough metadata to drive the dedicated
 * FlightHud and DerivedAttrs big-icon planners. The plain boolean flags exposed
 * in {@link com.bong.client.combat.DerivedAttrFlags} are kept in sync by the
 * handler for consumers that only need the minimal truth.
 */
public final class DerivedAttrsStore {

    public record State(
        boolean flying,
        float flyingQiRemaining,   // 0..1 (fraction of qi pool allocated to flight)
        long flyingForceDescentAtMs, // absolute ms; 0 if no warning
        boolean phasing,
        long phasingUntilMs,
        boolean tribulationLocked,
        String tribulationStage,    // "warn" / "striking" / "pending"
        float throughputPeakNorm,  // 0..1 peak qi throughput overlay on qi bar
        int vortexFakeSkinLayers,  // 0..3 for伪皮层
        boolean vortexReady
    ) {
        public static final State NONE = new State(
            false, 0f, 0L, false, 0L, false, "", 0f, 0, false
        );
    }

    private static volatile State snapshot = State.NONE;

    private DerivedAttrsStore() {}

    public static State snapshot() { return snapshot; }

    public static void replace(State next) {
        snapshot = next == null ? State.NONE : next;
    }

    public static void resetForTests() {
        snapshot = State.NONE;
    }
}
