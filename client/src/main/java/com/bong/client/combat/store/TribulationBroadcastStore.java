package com.bong.client.combat.store;

/**
 * Server-wide tribulation broadcast state (plan §U6 / §2.x).
 * A single broadcast is held at a time; last-write-wins.
 */
public final class TribulationBroadcastStore {

    public record State(
        boolean active,
        String actorName,
        String stage,              // "warn" / "locked" / "striking" / "done"
        double worldX,
        double worldZ,
        long expiresAtMs,
        boolean spectateInvite,     // within 50 blocks -> auto tip
        double spectateDistance
    ) {
        public State {
            actorName = actorName == null ? "" : actorName;
            stage = stage == null ? "" : stage;
        }

        public static final State NONE = new State(false, "", "", 0d, 0d, 0L, false, 0d);

        public boolean expired(long nowMs) {
            return expiresAtMs > 0L && nowMs >= expiresAtMs;
        }
    }

    private static volatile State snapshot = State.NONE;

    private TribulationBroadcastStore() {}

    public static State snapshot() { return snapshot; }

    public static void replace(State next) {
        snapshot = next == null ? State.NONE : next;
    }

    public static void clear() { snapshot = State.NONE; }

    public static void resetForTests() { snapshot = State.NONE; }
}
