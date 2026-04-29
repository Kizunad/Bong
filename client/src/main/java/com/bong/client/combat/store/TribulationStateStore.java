package com.bong.client.combat.store;

import java.util.List;

/**
 * Client-side mirror of the active tribulation phase and wave progress.
 */
public final class TribulationStateStore {
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

    private static volatile State snapshot = State.NONE;

    private TribulationStateStore() {}

    public static State snapshot() { return snapshot; }

    public static void replace(State next) {
        snapshot = next == null ? State.NONE : next;
    }

    public static void clear(State next) {
        snapshot = next == null ? State.NONE : next;
    }

    public static void resetForTests() { snapshot = State.NONE; }
}
