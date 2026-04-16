package com.bong.client.combat.store;

/**
 * Final-death termination screen state (plan §U4 / §2.3).
 */
public final class TerminateStateStore {

    public record State(
        boolean visible,
        String finalWords,
        String epilogue,
        String archetypeSuggestion
    ) {
        public State {
            finalWords = finalWords == null ? "" : finalWords;
            epilogue = epilogue == null ? "" : epilogue;
            archetypeSuggestion = archetypeSuggestion == null ? "" : archetypeSuggestion;
        }

        public static final State HIDDEN = new State(false, "", "", "");
    }

    private static volatile State snapshot = State.HIDDEN;

    private TerminateStateStore() {}

    public static State snapshot() { return snapshot; }

    public static void replace(State next) {
        snapshot = next == null ? State.HIDDEN : next;
    }

    public static void hide() { snapshot = State.HIDDEN; }

    public static void resetForTests() { snapshot = State.HIDDEN; }
}
