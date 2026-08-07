package com.bong.client.combat.store;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * Client-side mirror of the active tribulation phase and wave progress.
 */
public final class TribulationStateStore {
    private static final double PUBLIC_COORDINATE_GRID_BLOCKS = 200.0d;
    private static final String JUE_BI_PUBLIC_ACTOR = "\u7edd\u58c1\u52ab";

    public record State(
        boolean active,
        String charId,
        String actorName,
        String kind,
        String phase,
        double worldX,
        double worldZ,
        int waveCurrent,
        int waveTotal,
        long startedTick,
        long phaseStartedTick,
        long nextWaveTick,
        boolean failed,
        boolean halfStepOnSuccess,
        List<String> participants,
        String result
    ) {
        public State {
            charId = charId == null ? "" : charId;
            actorName = actorName == null ? "" : actorName;
            kind = kind == null ? "" : kind;
            phase = phase == null ? "" : phase;
            waveCurrent = Math.max(0, waveCurrent);
            waveTotal = Math.max(0, waveTotal);
            startedTick = Math.max(0L, startedTick);
            phaseStartedTick = Math.max(0L, phaseStartedTick);
            nextWaveTick = Math.max(0L, nextWaveTick);
            participants = participants == null ? List.of() : List.copyOf(participants);
            result = result == null ? "" : result;
        }

        public String key() {
            if (!charId.isBlank()) return charId;
            return actorName + "\u001f" + kind + "\u001f" + Double.toString(worldX) + "\u001f" + Double.toString(worldZ);
        }

        public boolean hasTargetKey() {
            return !charId.isBlank() || !actorName.isBlank()
                || Double.compare(worldX, 0d) != 0 || Double.compare(worldZ, 0d) != 0;
        }

        public static final State NONE = new State(
            false,
            "",
            "",
            "",
            "",
            0d,
            0d,
            0,
            0,
            0L,
            0L,
            0L,
            false,
            false,
            List.of(),
            ""
        );
    }

    private static volatile Map<String, State> snapshots = Map.of();
    private static volatile State lastTerminal = State.NONE;

    private TribulationStateStore() {}

    public static State snapshot() {
        State primary = selectPrimary(snapshots);
        return primary == State.NONE ? lastTerminal : primary;
    }

    public static State snapshotFor(String actorName, double worldX, double worldZ) {
        return selectForBroadcast(snapshots, actorName, worldX, worldZ);
    }

    public static List<State> all() { return List.copyOf(snapshots.values()); }

    public static void replace(State next) {
        if (next == null) {
            clear();
            return;
        }
        if (!next.active()) {
            clear(next);
            return;
        }
        LinkedHashMap<String, State> copy = new LinkedHashMap<>();
        copy.put(next.key(), next);
        snapshots = Collections.unmodifiableMap(copy);
        lastTerminal = State.NONE;
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
        lastTerminal = State.NONE;
    }

    public static void clear(State next) {
        if (next == null || !next.hasTargetKey()) {
            clear();
            lastTerminal = next == null ? State.NONE : next;
            return;
        }
        LinkedHashMap<String, State> copy = new LinkedHashMap<>(snapshots);
        copy.remove(next.key());
        snapshots = copy.isEmpty() ? Map.of() : Collections.unmodifiableMap(copy);
        lastTerminal = next;
    }

    public static void clear() {
        snapshots = Map.of();
        lastTerminal = State.NONE;
    }

    public static void clearOnDisconnect() {
        clear();
    }

    public static void resetForTests() { clear(); }

    private static State selectPrimary(Map<String, State> states) {
        State best = State.NONE;
        for (State candidate : states.values()) {
            if (!candidate.active()) continue;
            if (best == State.NONE || comparePrimary(candidate, best) < 0) {
                best = candidate;
            }
        }
        return best;
    }

    private static State selectForBroadcast(Map<String, State> states, String actorName, double worldX, double worldZ) {
        State best = State.NONE;
        String normalizedActor = actorName == null ? "" : actorName;
        for (State candidate : states.values()) {
            if (!candidate.active()) continue;
            if (!broadcastMatches(candidate, normalizedActor, worldX, worldZ)) continue;
            if (best == State.NONE || compareBroadcastMatch(candidate, best, normalizedActor, worldX, worldZ) < 0) {
                best = candidate;
            }
        }
        return best;
    }

    private static int compareBroadcastMatch(
        State a,
        State b,
        String actorName,
        double worldX,
        double worldZ
    ) {
        String normalizedActor = actorName == null ? "" : actorName;
        boolean aMatches = broadcastMatches(a, normalizedActor, worldX, worldZ);
        boolean bMatches = broadcastMatches(b, normalizedActor, worldX, worldZ);
        if (!aMatches && !bMatches) return 0;
        if (aMatches != bMatches) return aMatches ? -1 : 1;
        int cmp = Boolean.compare(actorMatches(b, normalizedActor), actorMatches(a, normalizedActor));
        if (cmp != 0) return cmp;
        cmp = Double.compare(distanceSquared(a, worldX, worldZ), distanceSquared(b, worldX, worldZ));
        if (cmp != 0) return cmp;
        return comparePrimary(a, b);
    }

    private static int comparePrimary(State a, State b) {
        int cmp = Integer.compare(phaseRank(b.phase()), phaseRank(a.phase()));
        if (cmp != 0) return cmp;
        cmp = Integer.compare(b.waveCurrent(), a.waveCurrent());
        if (cmp != 0) return cmp;
        cmp = a.actorName().compareTo(b.actorName());
        if (cmp != 0) return cmp;
        return a.charId().compareTo(b.charId());
    }

    private static boolean actorMatches(State state, String actorName) {
        return !actorName.isBlank() && actorName.equals(state.actorName());
    }

    private static boolean broadcastMatches(State state, String actorName, double worldX, double worldZ) {
        return actorMatches(state, actorName)
            || coordinateMatches(state, worldX, worldZ)
            || jueBiPublicBroadcastMatches(state, actorName, worldX, worldZ);
    }

    private static boolean jueBiPublicBroadcastMatches(State state, String actorName, double worldX, double worldZ) {
        return JUE_BI_PUBLIC_ACTOR.equals(actorName)
            && "jue_bi".equals(state.kind())
            && publicCoordinateMatches(state.worldX(), worldX)
            && publicCoordinateMatches(state.worldZ(), worldZ);
    }

    private static boolean coordinateMatches(State state, double worldX, double worldZ) {
        if (!Double.isFinite(worldX) || !Double.isFinite(worldZ)) return false;
        return nearlyEqual(state.worldX(), worldX) && nearlyEqual(state.worldZ(), worldZ);
    }

    private static boolean publicCoordinateMatches(double exactValue, double publicValue) {
        if (!Double.isFinite(exactValue) || !Double.isFinite(publicValue)) return false;
        return nearlyEqual(publicTribulationCoordinate(exactValue), publicValue);
    }

    private static double publicTribulationCoordinate(double value) {
        double scaled = value / PUBLIC_COORDINATE_GRID_BLOCKS;
        double rounded = scaled >= 0d ? Math.floor(scaled + 0.5d) : Math.ceil(scaled - 0.5d);
        return rounded * PUBLIC_COORDINATE_GRID_BLOCKS;
    }

    private static boolean nearlyEqual(double a, double b) {
        return Double.isFinite(a) && Double.isFinite(b) && Math.abs(a - b) <= 0.001d;
    }

    private static double distanceSquared(State state, double worldX, double worldZ) {
        if (!Double.isFinite(worldX) || !Double.isFinite(worldZ)) return Double.MAX_VALUE;
        double dx = state.worldX() - worldX;
        double dz = state.worldZ() - worldZ;
        return dx * dx + dz * dz;
    }

    private static int phaseRank(String phase) {
        return switch (phase == null ? "" : phase) {
            case "heart_demon" -> 4;
            case "wave" -> 3;
            case "lock" -> 2;
            case "omen" -> 1;
            default -> 0;
        };
    }
}
