package com.bong.client.combat.store;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * Server-wide tribulation broadcast state (plan §U6 / §2.x).
 * Multiple concurrent broadcasts are keyed by actor + public coordinates.
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

        public String key() {
            return keyFor(actorName, worldX, worldZ);
        }

        public boolean hasTargetKey() {
            return !actorName.isBlank() || Double.compare(worldX, 0d) != 0 || Double.compare(worldZ, 0d) != 0;
        }
    }

    private static volatile Map<String, State> snapshots = Map.of();

    private TribulationBroadcastStore() {}

    public static State snapshot() { return selectPrimary(snapshots, -1L); }

    public static State snapshot(long nowMs) { return selectPrimary(snapshots, nowMs); }

    public static List<State> all() { return List.copyOf(snapshots.values()); }

    public static void replace(State next) {
        if (next == null || !next.active()) {
            clear();
            return;
        }
        LinkedHashMap<String, State> copy = new LinkedHashMap<>();
        copy.put(next.key(), next);
        snapshots = Collections.unmodifiableMap(copy);
    }

    public static void upsert(State next) {
        if (next == null) {
            clear();
            return;
        }
        if (!next.active()) {
            clear(next);
            return;
        }
        LinkedHashMap<String, State> copy = new LinkedHashMap<>(snapshots);
        copy.put(next.key(), next);
        snapshots = Collections.unmodifiableMap(copy);
    }

    public static void clear(State target) {
        if (target == null || !target.hasTargetKey()) {
            clear();
            return;
        }
        LinkedHashMap<String, State> copy = new LinkedHashMap<>(snapshots);
        copy.remove(target.key());
        snapshots = copy.isEmpty() ? Map.of() : Collections.unmodifiableMap(copy);
    }

    public static void clear() { snapshots = Map.of(); }


    /** Clears session-scoped state while preserving process-lifetime wiring. */

    public static void clearOnDisconnect() {

        clear();

    }

    public static void resetForTests() { snapshots = Map.of(); }

    private static State selectPrimary(Map<String, State> states, long nowMs) {
        State best = State.NONE;
        for (State candidate : states.values()) {
            if (!candidate.active()) continue;
            if (nowMs >= 0L && candidate.expired(nowMs)) continue;
            if (best == State.NONE || comparePriority(candidate, best) < 0) {
                best = candidate;
            }
        }
        return best;
    }

    private static int comparePriority(State a, State b) {
        int cmp = Boolean.compare(b.spectateInvite(), a.spectateInvite());
        if (cmp != 0) return cmp;
        cmp = Integer.compare(stageRank(b.stage()), stageRank(a.stage()));
        if (cmp != 0) return cmp;
        cmp = Double.compare(safeDistance(a.spectateDistance()), safeDistance(b.spectateDistance()));
        if (cmp != 0) return cmp;
        cmp = Long.compare(b.expiresAtMs(), a.expiresAtMs());
        if (cmp != 0) return cmp;
        cmp = a.actorName().compareTo(b.actorName());
        if (cmp != 0) return cmp;
        cmp = Double.compare(a.worldX(), b.worldX());
        if (cmp != 0) return cmp;
        return Double.compare(a.worldZ(), b.worldZ());
    }

    private static int stageRank(String stage) {
        return switch (stage == null ? "" : stage) {
            case "striking" -> 3;
            case "locked" -> 2;
            case "warn" -> 1;
            default -> 0;
        };
    }

    private static double safeDistance(double value) {
        return Double.isFinite(value) && value >= 0d ? value : Double.MAX_VALUE;
    }

    private static String keyFor(String actorName, double worldX, double worldZ) {
        return (actorName == null ? "" : actorName)
            + "\u001f" + Double.toString(worldX)
            + "\u001f" + Double.toString(worldZ);
    }
}
